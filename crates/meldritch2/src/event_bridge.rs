//! Demand-driven, source-scoped events from real-time Rust producers to Lua.

use std::{
    cell::{RefCell, UnsafeCell},
    collections::{BTreeMap, VecDeque},
    mem::MaybeUninit,
    rc::Rc,
    sync::{
        Arc, Weak,
        atomic::{AtomicU8, AtomicU64, AtomicUsize, Ordering},
    },
};

use mlua::{Function, Lua, Table, UserData, UserDataMethods};

const EVENT_KIND_COUNT: usize = 5;
const QUEUE_CAPACITY: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum EventKind {
    StepAdvanced,
    NoteOn,
    NoteOff,
    VoiceTriggered,
    PatternChanged,
}

impl EventKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::StepAdvanced => "step_advanced",
            Self::NoteOn => "note_on",
            Self::NoteOff => "note_off",
            Self::VoiceTriggered => "voice_triggered",
            Self::PatternChanged => "pattern_changed",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "step_advanced" => Some(Self::StepAdvanced),
            "note_on" => Some(Self::NoteOn),
            "note_off" => Some(Self::NoteOff),
            "voice_triggered" => Some(Self::VoiceTriggered),
            "pattern_changed" => Some(Self::PatternChanged),
            _ => None,
        }
    }

    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EventPayload {
    Step {
        step: u8,
        pattern: u8,
        position: u64,
    },
    Note {
        track: u8,
        pitch: f64,
        velocity: f64,
    },
    Pattern {
        pattern: u8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimeEvent {
    pub source_id: u64,
    pub kind: EventKind,
    /// Absolute sample frame at which the event occurred in this source runtime.
    pub sample_position: u64,
    interest_epoch: u64,
    pub payload: EventPayload,
}

/// Fixed-storage SPSC queue. One compiled audio runtime publishes and the Lua
/// owner consumes. Full queues drop the newest observation and never block.
struct EventQueue {
    slots: [UnsafeCell<MaybeUninit<RuntimeEvent>>; QUEUE_CAPACITY],
    head: AtomicUsize,
    tail: AtomicUsize,
}

// The producer owns `head` and the consumer owns `tail`; release/acquire
// publication prevents either side from accessing the same slot concurrently.
unsafe impl Sync for EventQueue {}

impl EventQueue {
    fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| UnsafeCell::new(MaybeUninit::uninit())),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    fn try_push(&self, event: RuntimeEvent) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let next = (head + 1) % QUEUE_CAPACITY;
        if next == self.tail.load(Ordering::Acquire) {
            return false;
        }
        // SAFETY: only the producer writes the slot at `head`, and it cannot be
        // the consumer's current tail because the full condition was checked.
        unsafe { (*self.slots[head].get()).write(event) };
        self.head.store(next, Ordering::Release);
        true
    }

    fn pop(&self) -> Option<RuntimeEvent> {
        let tail = self.tail.load(Ordering::Relaxed);
        if tail == self.head.load(Ordering::Acquire) {
            return None;
        }
        // SAFETY: acquire of `head` observes the producer's initialized slot;
        // only this consumer reads and advances `tail`.
        let event = unsafe { (*self.slots[tail].get()).assume_init_read() };
        self.tail
            .store((tail + 1) % QUEUE_CAPACITY, Ordering::Release);
        Some(event)
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct EventDiagnostics {
    pub publications: u64,
    pub insertions: u64,
    pub dropped: u64,
}

/// Bounded UI observation state updated only by dispatched Lua callbacks. It
/// is not musical state and never feeds audio processing.
#[derive(Debug)]
pub struct EventUiState {
    step: AtomicU8,
    pattern: AtomicU8,
    position: AtomicU64,
    activity: [AtomicU64; 8],
    pitch: [AtomicU64; 8],
    velocity: [AtomicU64; 8],
}

impl Default for EventUiState {
    fn default() -> Self {
        Self {
            step: AtomicU8::new(0),
            pattern: AtomicU8::new(0),
            position: AtomicU64::new(0),
            activity: std::array::from_fn(|_| AtomicU64::new(0)),
            pitch: std::array::from_fn(|_| AtomicU64::new(440.0_f64.to_bits())),
            velocity: std::array::from_fn(|_| AtomicU64::new(0.0_f64.to_bits())),
        }
    }
}

impl EventUiState {
    pub fn step(&self) -> usize {
        usize::from(self.step.load(Ordering::Acquire))
    }
    pub fn pattern(&self) -> usize {
        usize::from(self.pattern.load(Ordering::Acquire))
    }
    pub fn position(&self) -> u64 {
        self.position.load(Ordering::Acquire)
    }
    pub fn activity(&self, track: usize) -> Option<(u64, f64, f64)> {
        Some((
            self.activity.get(track)?.load(Ordering::Acquire),
            f64::from_bits(self.pitch.get(track)?.load(Ordering::Acquire)),
            f64::from_bits(self.velocity.get(track)?.load(Ordering::Acquire)),
        ))
    }
    pub(crate) fn observe_step(&self, step: usize, pattern: usize, position: u64) {
        self.step
            .store(step.saturating_sub(1) as u8, Ordering::Release);
        self.pattern
            .store(pattern.saturating_sub(1) as u8, Ordering::Release);
        self.position.store(position, Ordering::Release);
    }
    pub(crate) fn observe_pattern(&self, pattern: usize) {
        self.pattern
            .store(pattern.saturating_sub(1) as u8, Ordering::Release);
    }
    pub(crate) fn observe_trigger(
        &self,
        track: usize,
        pitch: f64,
        velocity: f64,
    ) -> Result<(), String> {
        let index = track
            .checked_sub(1)
            .ok_or("event track must be at least 1")?;
        let generation = self
            .activity
            .get(index)
            .ok_or("event track must be 1..=8")?;
        self.pitch[index].store(pitch.to_bits(), Ordering::Release);
        self.velocity[index].store(velocity.to_bits(), Ordering::Release);
        generation.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LuaEventUiState(pub Arc<EventUiState>);

impl UserData for LuaEventUiState {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method(
            "step",
            |_, this, (step, pattern, position): (usize, usize, u64)| {
                this.0.observe_step(step, pattern, position);
                Ok(())
            },
        );
        methods.add_method("pattern", |_, this, pattern: usize| {
            this.0.observe_pattern(pattern);
            Ok(())
        });
        methods.add_method(
            "trigger",
            |_, this, (track, pitch, velocity): (usize, f64, f64)| {
                this.0
                    .observe_trigger(track, pitch, velocity)
                    .map_err(mlua::Error::external)
            },
        );
    }
}

pub struct EventSource {
    id: u64,
    name: String,
    supported: usize,
    interest: [AtomicUsize; EVENT_KIND_COUNT],
    interest_epoch: [AtomicU64; EVENT_KIND_COUNT],
    queue: EventQueue,
    publications: AtomicU64,
    insertions: AtomicU64,
    dropped: AtomicU64,
}

impl std::fmt::Debug for EventSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventSource")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl EventSource {
    pub fn new(name: String, kinds: &[EventKind]) -> Arc<Self> {
        static NEXT_SOURCE: AtomicU64 = AtomicU64::new(1);
        let supported = kinds
            .iter()
            .fold(0usize, |mask, kind| mask | (1 << kind.index()));
        Arc::new(Self {
            id: NEXT_SOURCE.fetch_add(1, Ordering::Relaxed),
            name,
            supported,
            interest: std::array::from_fn(|_| AtomicUsize::new(0)),
            interest_epoch: std::array::from_fn(|_| AtomicU64::new(0)),
            queue: EventQueue::new(),
            publications: AtomicU64::new(0),
            insertions: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
        })
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn supports(&self, kind: EventKind) -> bool {
        self.supported & (1 << kind.index()) != 0
    }

    pub fn event_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        [
            EventKind::StepAdvanced,
            EventKind::NoteOn,
            EventKind::NoteOff,
            EventKind::VoiceTriggered,
            EventKind::PatternChanged,
        ]
        .into_iter()
        .filter(|kind| self.supports(*kind))
        .map(EventKind::name)
    }

    fn add_interest(&self, kind: EventKind) -> u64 {
        let count = &self.interest[kind.index()];
        if count.load(Ordering::Acquire) == 0 {
            let epoch = self.interest_epoch[kind.index()].fetch_add(1, Ordering::AcqRel) + 1;
            count.store(1, Ordering::Release);
            epoch
        } else {
            count.fetch_add(1, Ordering::AcqRel);
            self.interest_epoch[kind.index()].load(Ordering::Acquire)
        }
    }

    fn remove_interest(&self, kind: EventKind) {
        let count = &self.interest[kind.index()];
        let _ = count.try_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value.checked_sub(1)
        });
    }

    pub fn interested(&self, kind: EventKind) -> bool {
        self.interest[kind.index()].load(Ordering::Acquire) != 0
    }

    fn current_interest_epoch(&self, kind: EventKind) -> Option<u64> {
        let index = kind.index();
        let before = self.interest_epoch[index].load(Ordering::Acquire);
        if self.interest[index].load(Ordering::Acquire) == 0 {
            return None;
        }
        let after = self.interest_epoch[index].load(Ordering::Acquire);
        (before == after).then_some(after)
    }

    /// The payload closure is not evaluated unless this exact source/kind has
    /// interest. The interested path is fixed-size atomic queue work only.
    pub fn publish(
        &self,
        kind: EventKind,
        sample_position: u64,
        payload: impl FnOnce() -> EventPayload,
    ) {
        let Some(interest_epoch) = self.current_interest_epoch(kind) else {
            return;
        };
        self.publications.fetch_add(1, Ordering::Relaxed);
        let event = RuntimeEvent {
            source_id: self.id,
            kind,
            sample_position,
            interest_epoch,
            payload: payload(),
        };
        if self.queue.try_push(event) {
            self.insertions.fetch_add(1, Ordering::Relaxed);
        } else {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn diagnostics(&self) -> EventDiagnostics {
        EventDiagnostics {
            publications: self.publications.load(Ordering::Relaxed),
            insertions: self.insertions.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
        }
    }
}

struct Subscription {
    source: Weak<EventSource>,
    source_id: u64,
    kind: EventKind,
    interest_epoch: u64,
    callback: Function,
}

#[derive(Default)]
struct Registry {
    next_id: u64,
    subscriptions: BTreeMap<u64, Subscription>,
    sources: BTreeMap<u64, Weak<EventSource>>,
    errors: VecDeque<String>,
    callbacks: u64,
    failures: u64,
}

impl Registry {
    fn connect(&mut self, source: &Arc<EventSource>, kind: EventKind, callback: Function) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        let interest_epoch = source.add_interest(kind);
        self.sources.insert(source.id(), Arc::downgrade(source));
        self.subscriptions.insert(
            id,
            Subscription {
                source: Arc::downgrade(source),
                source_id: source.id(),
                kind,
                interest_epoch,
                callback,
            },
        );
        id
    }

    fn disconnect(&mut self, id: u64) -> bool {
        let Some(subscription) = self.subscriptions.remove(&id) else {
            return false;
        };
        if let Some(source) = subscription.source.upgrade() {
            source.remove_interest(subscription.kind);
        }
        true
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        for subscription in self.subscriptions.values() {
            if let Some(source) = subscription.source.upgrade() {
                source.remove_interest(subscription.kind);
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct SubscriptionRegistry(Rc<RefCell<Registry>>);

impl Default for SubscriptionRegistry {
    fn default() -> Self {
        Self(Rc::new(RefCell::new(Registry::default())))
    }
}

impl SubscriptionRegistry {
    pub(crate) fn connect(
        &self,
        source: &Arc<EventSource>,
        event: &str,
        callback: Function,
    ) -> Result<u64, String> {
        let kind = EventKind::parse(event).ok_or_else(|| format!("unknown event '{event}'"))?;
        if !source.supports(kind) {
            return Err(format!(
                "source '{}' does not produce event '{event}'",
                source.name()
            ));
        }
        Ok(self.0.borrow_mut().connect(source, kind, callback))
    }

    pub(crate) fn disconnect(&self, id: u64) -> bool {
        self.0.borrow_mut().disconnect(id)
    }
}

pub(crate) struct LuaSubscription {
    registry: SubscriptionRegistry,
    id: Option<u64>,
}

impl LuaSubscription {
    pub(crate) fn new(registry: SubscriptionRegistry, id: u64) -> Self {
        Self {
            registry,
            id: Some(id),
        }
    }

    fn disconnect(&mut self) -> bool {
        self.id
            .take()
            .is_some_and(|id| self.registry.disconnect(id))
    }
}

impl Drop for LuaSubscription {
    fn drop(&mut self) {
        self.disconnect();
    }
}

impl UserData for LuaSubscription {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("disconnect", |_, this, ()| Ok(this.disconnect()));
        methods.add_method("connected", |_, this, ()| Ok(this.id.is_some()));
    }
}

pub(crate) struct ScreenSubscription {
    pub screen: String,
    pub source: Weak<EventSource>,
    pub kind: EventKind,
    pub callback: Function,
}

pub(crate) struct ScreenLifecycle {
    pub screen: String,
    pub mount: Option<Function>,
    pub unmount: Option<Function>,
}

/// Owned by the UI thread together with the retained authoring Lua state.
pub struct EventRuntime {
    registry: SubscriptionRegistry,
    screen_specs: Vec<ScreenSubscription>,
    lifecycle: Vec<ScreenLifecycle>,
    mounted: BTreeMap<String, Vec<u64>>,
}

impl EventRuntime {
    pub(crate) fn new(
        registry: SubscriptionRegistry,
        screen_specs: Vec<ScreenSubscription>,
        lifecycle: Vec<ScreenLifecycle>,
    ) -> Self {
        Self {
            registry,
            screen_specs,
            lifecycle,
            mounted: BTreeMap::new(),
        }
    }

    pub fn mount_screen(&mut self, screen: &str) -> Result<(), String> {
        if self.mounted.contains_key(screen) {
            return Ok(());
        }
        let mut ids = Vec::new();
        for spec in self
            .screen_specs
            .iter()
            .filter(|spec| spec.screen == screen)
        {
            let source = spec
                .source
                .upgrade()
                .ok_or_else(|| format!("event source for screen '{screen}' was destroyed"))?;
            ids.push(self.registry.0.borrow_mut().connect(
                &source,
                spec.kind,
                spec.callback.clone(),
            ));
        }
        self.mounted.insert(screen.to_owned(), ids);
        if let Some(callback) = self
            .lifecycle
            .iter()
            .find(|lifecycle| lifecycle.screen == screen)
            .and_then(|lifecycle| lifecycle.mount.as_ref())
        {
            callback
                .call::<()>(())
                .map_err(|error| format!("screen '{screen}' mount callback: {error}"))?;
        }
        Ok(())
    }

    pub fn unmount_screen(&mut self, screen: &str) {
        let Some(ids) = self.mounted.remove(screen) else {
            return;
        };
        for id in ids {
            self.registry.disconnect(id);
        }
        if let Some(callback) = self
            .lifecycle
            .iter()
            .find(|lifecycle| lifecycle.screen == screen)
            .and_then(|lifecycle| lifecycle.unmount.as_ref())
            && let Err(error) = callback.call::<()>(())
        {
            let mut registry = self.registry.0.borrow_mut();
            registry.failures += 1;
            if registry.errors.len() == 32 {
                registry.errors.pop_front();
            }
            registry
                .errors
                .push_back(format!("screen '{screen}' unmount callback: {error}"));
        }
    }

    pub fn dispatch(&mut self, lua: &Lua) -> usize {
        let dead = self
            .registry
            .0
            .borrow()
            .subscriptions
            .iter()
            .filter_map(|(id, subscription)| subscription.source.upgrade().is_none().then_some(*id))
            .collect::<Vec<_>>();
        for id in dead {
            self.registry.disconnect(id);
        }
        let sources = self
            .registry
            .0
            .borrow()
            .sources
            .iter()
            .filter_map(|(id, source)| source.upgrade().map(|source| (*id, source)))
            .collect::<Vec<_>>();
        let mut delivered = 0;
        for (source_id, source) in sources {
            while let Some(event) = source.queue.pop() {
                let callbacks = self
                    .registry
                    .0
                    .borrow()
                    .subscriptions
                    .values()
                    .filter(|subscription| {
                        subscription.source_id == source_id
                            && subscription.kind == event.kind
                            && subscription.interest_epoch == event.interest_epoch
                    })
                    .map(|subscription| subscription.callback.clone())
                    .collect::<Vec<_>>();
                for callback in callbacks {
                    let result = event_table(lua, &source, event)
                        .and_then(|table| callback.call::<()>(table));
                    if let Err(error) = result {
                        let mut registry = self.registry.0.borrow_mut();
                        registry.failures += 1;
                        if registry.errors.len() == 32 {
                            registry.errors.pop_front();
                        }
                        registry.errors.push_back(format!(
                            "source '{}' event '{}': {error}",
                            source.name(),
                            event.kind.name()
                        ));
                    } else {
                        delivered += 1;
                        self.registry.0.borrow_mut().callbacks += 1;
                    }
                }
            }
        }
        delivered
    }

    pub fn take_errors(&mut self) -> Vec<String> {
        self.registry.0.borrow_mut().errors.drain(..).collect()
    }

    pub fn callback_count(&self) -> u64 {
        self.registry.0.borrow().callbacks
    }

    pub fn failure_count(&self) -> u64 {
        self.registry.0.borrow().failures
    }

    pub fn active_subscriptions(&self) -> usize {
        self.registry.0.borrow().subscriptions.len()
    }
}

fn event_table(lua: &Lua, source: &EventSource, event: RuntimeEvent) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("kind", event.kind.name())?;
    table.set("source_id", source.id())?;
    table.set("source", source.name())?;
    table.set("sample_position", event.sample_position)?;
    match event.payload {
        EventPayload::Step {
            step,
            pattern,
            position,
        } => {
            table.set("step", usize::from(step) + 1)?;
            table.set("pattern", usize::from(pattern) + 1)?;
            table.set("position", position)?;
        }
        EventPayload::Note {
            track,
            pitch,
            velocity,
        } => {
            table.set("track", usize::from(track) + 1)?;
            table.set("pitch", pitch)?;
            table.set("velocity", velocity)?;
        }
        EventPayload::Pattern { pattern } => {
            table.set("pattern", usize::from(pattern) + 1)?;
        }
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequencer::{SequencerControl, SequencerRuntime};
    use std::{cell::RefCell, rc::Rc, thread};

    #[test]
    fn no_interest_does_not_construct_or_queue_payload() {
        let source = EventSource::new("zero".into(), &[EventKind::NoteOn]);
        let constructed = AtomicUsize::new(0);
        source.publish(EventKind::NoteOn, 12, || {
            constructed.fetch_add(1, Ordering::Relaxed);
            EventPayload::Note {
                track: 0,
                pitch: 440.0,
                velocity: 1.0,
            }
        });
        assert_eq!(constructed.load(Ordering::Relaxed), 0);
        assert_eq!(source.diagnostics(), EventDiagnostics::default());
        assert!(source.queue.pop().is_none());
    }

    fn note(track: u8, pitch: f64) -> EventPayload {
        EventPayload::Note {
            track,
            pitch,
            velocity: 0.75,
        }
    }

    #[test]
    fn real_sequencer_without_subscribers_keeps_playing_without_bridge_work() {
        let control = SequencerControl::new("zero-sequence".into(), 120.0, 16, 4).unwrap();
        control.add_track("voice".into()).unwrap();
        let mut steps = [0.0; 16];
        steps[0] = 1.0;
        control.set_steps(0, &steps).unwrap();
        let mut runtime = SequencerRuntime::new(Arc::clone(&control));
        runtime.begin_frame(48_000.0);
        assert_eq!(runtime.trigger(0), 1.0);
        assert_eq!(
            control.event_source().diagnostics(),
            EventDiagnostics::default()
        );
    }

    #[test]
    fn source_kind_multiplicity_unsubscription_order_and_thread_are_explicit() {
        let lua = Lua::new();
        let registry = SubscriptionRegistry::default();
        let source_a = EventSource::new("A".into(), &[EventKind::NoteOn, EventKind::StepAdvanced]);
        let source_b = EventSource::new("B".into(), &[EventKind::NoteOn]);
        let observed = Rc::new(RefCell::new(Vec::new()));
        let dispatch_thread = thread::current().id();
        let callback = |observed: Rc<RefCell<Vec<(u64, thread::ThreadId)>>>| {
            lua.create_function(move |_, event: Table| {
                observed
                    .borrow_mut()
                    .push((event.get("sample_position")?, thread::current().id()));
                Ok(())
            })
            .unwrap()
        };
        let first = registry
            .connect(&source_a, "note_on", callback(Rc::clone(&observed)))
            .unwrap();
        let _second = registry
            .connect(&source_a, "note_on", callback(Rc::clone(&observed)))
            .unwrap();
        assert!(source_a.interested(EventKind::NoteOn));
        assert!(!source_a.interested(EventKind::StepAdvanced));
        assert!(!source_b.interested(EventKind::NoteOn));

        let producer_source = Arc::clone(&source_a);
        thread::scope(|scope| {
            scope.spawn(move || {
                producer_source.publish(EventKind::NoteOn, 41, || note(0, 220.0));
                producer_source.publish(EventKind::NoteOn, 42, || note(0, 330.0));
            });
        });
        let mut runtime = EventRuntime::new(registry.clone(), Vec::new(), Vec::new());
        assert_eq!(runtime.dispatch(&lua), 4);
        assert_eq!(source_a.diagnostics().publications, 2);
        assert_eq!(source_b.diagnostics(), EventDiagnostics::default());
        assert_eq!(
            observed
                .borrow()
                .iter()
                .map(|item| item.0)
                .collect::<Vec<_>>(),
            [41, 41, 42, 42]
        );
        assert!(
            observed
                .borrow()
                .iter()
                .all(|item| item.1 == dispatch_thread)
        );

        assert!(registry.disconnect(first));
        assert!(!registry.disconnect(first));
        source_a.publish(EventKind::NoteOn, 43, || note(0, 440.0));
        assert_eq!(runtime.dispatch(&lua), 1);
    }

    #[test]
    fn overflow_and_callback_failures_are_observable_without_stopping_dispatch() {
        let lua = Lua::new();
        let registry = SubscriptionRegistry::default();
        let source = EventSource::new("bounded".into(), &[EventKind::NoteOn]);
        let successes = Rc::new(RefCell::new(0usize));
        registry
            .connect(
                &source,
                "note_on",
                lua.create_function(|_, _: Table| -> mlua::Result<()> {
                    Err(mlua::Error::external("subscriber exploded"))
                })
                .unwrap(),
            )
            .unwrap();
        let count = Rc::clone(&successes);
        registry
            .connect(
                &source,
                "note_on",
                lua.create_function(move |_, _: Table| {
                    *count.borrow_mut() += 1;
                    Ok(())
                })
                .unwrap(),
            )
            .unwrap();
        for sample in 0..(QUEUE_CAPACITY as u64 + 20) {
            source.publish(EventKind::NoteOn, sample, || note(0, 440.0));
        }
        assert!(source.diagnostics().dropped > 0);
        let mut runtime = EventRuntime::new(registry, Vec::new(), Vec::new());
        runtime.dispatch(&lua);
        assert!(*successes.borrow() > 0);
        assert!(runtime.take_errors()[0].contains("bounded"));
        source.publish(EventKind::NoteOn, 999, || note(0, 440.0));
        runtime.dispatch(&lua);
        assert!(*successes.borrow() > 1);
    }

    #[test]
    fn destroyed_sources_are_pruned_without_stale_dispatch() {
        let lua = Lua::new();
        let registry = SubscriptionRegistry::default();
        let source = EventSource::new("temporary".into(), &[EventKind::NoteOn]);
        registry
            .connect(
                &source,
                "note_on",
                lua.create_function(|_, _: Table| Ok(())).unwrap(),
            )
            .unwrap();
        let mut runtime = EventRuntime::new(registry, Vec::new(), Vec::new());
        assert_eq!(runtime.active_subscriptions(), 1);
        drop(source);
        runtime.dispatch(&lua);
        assert_eq!(runtime.active_subscriptions(), 0);
    }

    #[test]
    fn sequencer_note_edges_and_committed_pattern_have_sample_timestamps() {
        let lua = Lua::new();
        let registry = SubscriptionRegistry::default();
        let control = SequencerControl::new("timing".into(), 120.0, 16, 4).unwrap();
        control.add_track("voice".into()).unwrap();
        let first = control.define_pattern("first".into()).unwrap();
        let second = control.define_pattern("second".into()).unwrap();
        let mut pitch = [440.0; 16];
        pitch[0] = 220.0;
        let gate = [0.5; 16];
        let mut velocity = [0.0; 16];
        velocity[0] = 1.0;
        control
            .set_pattern_notes(first, 0, &pitch, &gate, &velocity)
            .unwrap();
        control.queue_pattern(second).unwrap();
        let observed = Rc::new(RefCell::new(Vec::new()));
        for name in ["note_on", "note_off", "pattern_changed"] {
            let observed = Rc::clone(&observed);
            registry
                .connect(
                    control.event_source(),
                    name,
                    lua.create_function(move |_, event: Table| {
                        observed.borrow_mut().push((
                            event.get::<String>("kind")?,
                            event.get::<u64>("sample_position")?,
                        ));
                        Ok(())
                    })
                    .unwrap(),
                )
                .unwrap();
        }
        let mut sequencer = SequencerRuntime::new(Arc::clone(&control));
        for _ in 0..17 {
            sequencer.begin_frame(8.0);
        }
        let mut runtime = EventRuntime::new(registry, Vec::new(), Vec::new());
        runtime.dispatch(&lua);
        assert_eq!(
            observed.borrow().as_slice(),
            [
                ("note_on".into(), 0),
                ("note_off".into(), 1),
                ("pattern_changed".into(), 16),
            ]
        );
    }

    #[test]
    fn registry_shutdown_removes_interest_from_outstanding_handles() {
        let lua = Lua::new();
        let source = EventSource::new("shutdown".into(), &[EventKind::NoteOn]);
        {
            let registry = SubscriptionRegistry::default();
            registry
                .connect(
                    &source,
                    "note_on",
                    lua.create_function(|_, _: Table| Ok(())).unwrap(),
                )
                .unwrap();
            assert!(source.interested(EventKind::NoteOn));
        }
        assert!(!source.interested(EventKind::NoteOn));
    }

    #[test]
    fn a_new_subscription_cannot_receive_an_old_lifetime_event() {
        let lua = Lua::new();
        let registry = SubscriptionRegistry::default();
        let source = EventSource::new("lifetime".into(), &[EventKind::NoteOn]);
        let old = registry
            .connect(
                &source,
                "note_on",
                lua.create_function(|_, _: Table| Ok(())).unwrap(),
            )
            .unwrap();
        source.publish(EventKind::NoteOn, 1, || note(0, 220.0));
        registry.disconnect(old);
        let calls = Rc::new(RefCell::new(0));
        let count = Rc::clone(&calls);
        registry
            .connect(
                &source,
                "note_on",
                lua.create_function(move |_, _: Table| {
                    *count.borrow_mut() += 1;
                    Ok(())
                })
                .unwrap(),
            )
            .unwrap();
        let mut runtime = EventRuntime::new(registry, Vec::new(), Vec::new());
        assert_eq!(runtime.dispatch(&lua), 0);
        assert_eq!(*calls.borrow(), 0);
        source.publish(EventKind::NoteOn, 2, || note(0, 330.0));
        assert_eq!(runtime.dispatch(&lua), 1);
    }
}
