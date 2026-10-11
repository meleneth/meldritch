#[path = "../src/plaits.rs"]
mod plaits;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}
struct ObservedAllocator;
unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNTING.with(|on| {
            if on.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        COUNTING.with(|on| {
            if on.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;
#[test]
fn construction_and_processing_allocate_nothing_for_supported_engines() {
    use meldritch_dsl::{PlaitsModel, PlaitsSettings};
    for model in [PlaitsModel::Fm, PlaitsModel::Harmonic] {
        ALLOCATIONS.with(|n| n.set(0));
        COUNTING.with(|on| on.set(true));
        let mut source = plaits::PlaitsSource::new(
            PlaitsSettings {
                model,
                ..PlaitsSettings::default()
            },
            false,
        );
        for _ in 0..48000 {
            std::hint::black_box(source.next(440.0));
        }
        COUNTING.with(|on| on.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    }
}
