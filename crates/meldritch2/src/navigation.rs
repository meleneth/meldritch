use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use mlua::{Function, Lua, RegistryKey, Table};

use crate::model::Navigation;

const STATE_MACHINE: &str = include_str!("../vendor/lua-state-machine/statemachine.lua");

/// UI-thread Lua state machine. Rust only delivers named key events and applies
/// the screen identifier selected by Lua's transition callback.
pub(crate) struct NavigationRuntime {
    lua: Lua,
    fsm: RegistryKey,
    bindings: BTreeMap<String, String>,
    requested_screen: Rc<RefCell<Option<String>>>,
    initial_screen: String,
}

impl NavigationRuntime {
    pub fn new(navigation: &Navigation) -> Result<Self, String> {
        let lua = Lua::new();
        let machine: Table = lua
            .load(STATE_MACHINE)
            .set_name("@vendor/lua-state-machine/statemachine.lua")
            .eval()
            .map_err(|error| format!("failed to load Lua navigation FSM: {error}"))?;
        let options = lua.create_table().map_err(|e| e.to_string())?;
        options
            .set("initial", navigation.initial.as_str())
            .map_err(|e| e.to_string())?;
        let events = lua.create_table().map_err(|e| e.to_string())?;
        for (index, authored) in navigation.events.iter().enumerate() {
            let event = lua.create_table().map_err(|e| e.to_string())?;
            event
                .set("name", authored.name.as_str())
                .map_err(|e| e.to_string())?;
            event
                .set("from", authored.from.as_str())
                .map_err(|e| e.to_string())?;
            event
                .set("to", authored.to.as_str())
                .map_err(|e| e.to_string())?;
            events.set(index + 1, event).map_err(|e| e.to_string())?;
        }
        options.set("events", events).map_err(|e| e.to_string())?;

        let requested_screen = Rc::new(RefCell::new(None));
        let request = Rc::clone(&requested_screen);
        let on_change = lua
            .create_function(
                move |_, (_fsm, _event, _from, to): (Table, String, String, String)| {
                    *request.borrow_mut() = Some(to);
                    Ok(())
                },
            )
            .map_err(|e| e.to_string())?;
        let callbacks = lua.create_table().map_err(|e| e.to_string())?;
        callbacks
            .set("onstatechange", on_change)
            .map_err(|e| e.to_string())?;
        options
            .set("callbacks", callbacks)
            .map_err(|e| e.to_string())?;

        let create: Function = machine.get("create").map_err(|e| e.to_string())?;
        let fsm: Table = create.call(options).map_err(|e| e.to_string())?;
        let fsm = lua.create_registry_value(fsm).map_err(|e| e.to_string())?;
        let bindings = navigation
            .bindings
            .iter()
            .map(|binding| (binding.key.clone(), binding.event.clone()))
            .collect();
        Ok(Self {
            lua,
            fsm,
            bindings,
            requested_screen,
            initial_screen: navigation.initial.clone(),
        })
    }

    pub fn initial_screen(&self) -> &str {
        &self.initial_screen
    }

    /// Returns `None` for an unbound key so unrelated input remains available.
    pub fn dispatch(&self, key: &str) -> Result<Option<String>, String> {
        let Some(event) = self.bindings.get(key) else {
            return Ok(None);
        };
        *self.requested_screen.borrow_mut() = None;
        let fsm: Table = self
            .lua
            .registry_value(&self.fsm)
            .map_err(|e| e.to_string())?;
        let transition: Function = fsm.get(event.as_str()).map_err(|e| e.to_string())?;
        let changed: bool = transition
            .call(fsm.clone())
            .map_err(|e| format!("Lua navigation event '{event}' failed: {e}"))?;
        if !changed {
            return Err(format!(
                "Lua navigation event '{event}' is not valid from the current screen"
            ));
        }
        self.requested_screen
            .borrow_mut()
            .take()
            .ok_or_else(|| format!("Lua navigation event '{event}' did not activate a screen"))
            .map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::NavigationRuntime;
    use crate::model::{Navigation, NavigationBinding, NavigationEvent};

    #[test]
    fn lua_fsm_owns_transitions_and_ignores_unbound_keys() {
        let navigation = Navigation {
            initial: "alpha".into(),
            events: vec![
                NavigationEvent {
                    name: "alpha".into(),
                    from: "*".into(),
                    to: "alpha".into(),
                },
                NavigationEvent {
                    name: "beta".into(),
                    from: "*".into(),
                    to: "beta".into(),
                },
            ],
            bindings: vec![
                NavigationBinding {
                    key: "F1".into(),
                    event: "alpha".into(),
                },
                NavigationBinding {
                    key: "F2".into(),
                    event: "beta".into(),
                },
            ],
        };
        let runtime = NavigationRuntime::new(&navigation).unwrap();
        assert_eq!(runtime.initial_screen(), "alpha");
        assert_eq!(runtime.dispatch("x").unwrap(), None);
        assert_eq!(runtime.dispatch("F2").unwrap().as_deref(), Some("beta"));
        assert_eq!(runtime.dispatch("F2").unwrap().as_deref(), Some("beta"));
        assert_eq!(runtime.dispatch("F1").unwrap().as_deref(), Some("alpha"));
    }
}
