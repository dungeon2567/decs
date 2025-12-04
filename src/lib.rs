#![feature(allocator_api)]

extern crate self as decs;

pub mod component;
pub mod entity;
pub mod ecs;
pub mod rollback;
pub mod pool;
pub mod scheduler;
pub mod storage;
pub mod system;
pub mod frame;
pub mod tick;
pub mod view;
pub mod world;

// Re-export macros
pub use decs_macros::system;
