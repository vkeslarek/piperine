//! A run's memory under the counting allocator: the peak live heap of a run does not depend on its length, and an array of carriers costs what is written. The allocator is the whole process's, so these tests are a target of their own: no other test runs beside them. Each former target is a module of this one.

mod carrier_array_storage;
mod memory;

/// The counting allocator of the target: one for the process, shared by every test that measures.
#[global_allocator]
static HEAP: piperine_heap_fixtures::HeapCounter = piperine_heap_fixtures::HeapCounter::new();
