#![feature(allocator_api)]
#![allow(non_upper_case_globals)]

extern crate self as decs;

pub mod component;
pub mod ecs;
pub mod entity;
pub mod frame;
pub mod pool;
pub mod rollback;
pub mod scheduler;
pub mod storage;
pub mod system;
pub mod tick;
pub mod view;
pub mod world;

// Re-export macros
pub use decs_macros::system;
