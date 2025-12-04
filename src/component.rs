use crate::world::World;
use decs::system::TemporaryComponentCleanupSystem;

pub trait Component
where
    Self: Sized + 'static,
{
    fn id() -> u32;

    fn initialize(_id: u32) {}

    fn default_page() -> &'static crate::storage::Page<Self>;
    fn default_chunk() -> &'static crate::storage::Chunk<Self>;

    fn schedule_cleanup_system(world: &mut World);
}

#[derive(Clone)]
pub struct Destroyed();

#[allow(non_upper_case_globals)]
static mut __DECS_COMPONENT_ID_Destroyed: u32 = u32::MAX;

// impl is provided later with default providers

#[allow(non_snake_case)]
mod __decs_component_defaults_Destroyed {
    pub(super) static mut DEFAULT_PAGE: *const crate::storage::Page<crate::component::Destroyed> =
        std::ptr::null();
    pub(super) static mut DEFAULT_CHUNK: *const crate::storage::Chunk<crate::component::Destroyed> =
        std::ptr::null();
}

impl Destroyed {
    pub fn __decs_default_page() -> &'static crate::storage::Page<crate::component::Destroyed> {
        unsafe {
            if self::__decs_component_defaults_Destroyed::DEFAULT_PAGE.is_null()
                || self::__decs_component_defaults_Destroyed::DEFAULT_CHUNK.is_null()
            {
                let default_chunk_box =
                    Box::new(crate::storage::Chunk::<crate::component::Destroyed>::new());
                let default_chunk_ptr_mut: *mut crate::storage::Chunk<crate::component::Destroyed> =
                    Box::leak(default_chunk_box);
                let default_chunk_ptr: *const crate::storage::Chunk<crate::component::Destroyed> =
                    default_chunk_ptr_mut as *const _;
                let page_box = Box::new(crate::storage::Page::<crate::component::Destroyed> {
                    presence_mask: 0,
                    fullness_mask: 0,
                    changed_mask: 0,
                    count: 0,
                    data: [default_chunk_ptr_mut; 64],
                    chunk_pool: std::ptr::null_mut(),
                    pool_slot: 0,
                    pool_page: std::ptr::null_mut(),
                    owner_index: 0,
                });
                self::__decs_component_defaults_Destroyed::DEFAULT_PAGE =
                    Box::leak(page_box) as *const crate::storage::Page<crate::component::Destroyed>;
                self::__decs_component_defaults_Destroyed::DEFAULT_CHUNK = default_chunk_ptr;
            }
            &*self::__decs_component_defaults_Destroyed::DEFAULT_PAGE
        }
    }
    pub fn __decs_default_chunk() -> &'static crate::storage::Chunk<crate::component::Destroyed> {
        unsafe {
            if self::__decs_component_defaults_Destroyed::DEFAULT_CHUNK.is_null()
                || self::__decs_component_defaults_Destroyed::DEFAULT_PAGE.is_null()
            {
                let default_chunk_box =
                    Box::new(crate::storage::Chunk::<crate::component::Destroyed>::new());
                let default_chunk_ptr_mut: *mut crate::storage::Chunk<crate::component::Destroyed> =
                    Box::leak(default_chunk_box);
                let default_chunk_ptr: *const crate::storage::Chunk<crate::component::Destroyed> =
                    default_chunk_ptr_mut as *const _;
                let page_box = Box::new(crate::storage::Page::<crate::component::Destroyed> {
                    presence_mask: 0,
                    fullness_mask: 0,
                    changed_mask: 0,
                    count: 0,
                    data: [default_chunk_ptr_mut; 64],
                    chunk_pool: std::ptr::null_mut(),
                    pool_slot: 0,
                    pool_page: std::ptr::null_mut(),
                    owner_index: 0,
                });
                self::__decs_component_defaults_Destroyed::DEFAULT_PAGE =
                    Box::leak(page_box) as *const crate::storage::Page<crate::component::Destroyed>;
                self::__decs_component_defaults_Destroyed::DEFAULT_CHUNK = default_chunk_ptr;
            }
            &*self::__decs_component_defaults_Destroyed::DEFAULT_CHUNK
        }
    }
}

impl Component for Destroyed {
    fn default_page() -> &'static crate::storage::Page<Destroyed> {
        Destroyed::__decs_default_page()
    }
    fn default_chunk() -> &'static crate::storage::Chunk<Destroyed> {
        Destroyed::__decs_default_chunk()
    }
    fn id() -> u32 {
        unsafe { __DECS_COMPONENT_ID_Destroyed }
    }
    fn initialize(id: u32) {
        unsafe {
            if __DECS_COMPONENT_ID_Destroyed == u32::MAX {
                __DECS_COMPONENT_ID_Destroyed = id;
            }
            if self::__decs_component_defaults_Destroyed::DEFAULT_CHUNK.is_null()
                || self::__decs_component_defaults_Destroyed::DEFAULT_PAGE.is_null()
            {
                let default_chunk_box =
                    Box::new(crate::storage::Chunk::<crate::component::Destroyed>::new());
                let default_chunk_ptr_mut: *mut crate::storage::Chunk<crate::component::Destroyed> =
                    Box::leak(default_chunk_box);
                let default_chunk_ptr: *const crate::storage::Chunk<crate::component::Destroyed> =
                    default_chunk_ptr_mut as *const _;
                let page_box = Box::new(crate::storage::Page::<crate::component::Destroyed> {
                    presence_mask: 0,
                    fullness_mask: 0,
                    changed_mask: 0,
                    count: 0,
                    data: [default_chunk_ptr_mut; 64],
                    chunk_pool: std::ptr::null_mut(),
                    pool_slot: 0,
                    pool_page: std::ptr::null_mut(),
                    owner_index: 0,
                });
                self::__decs_component_defaults_Destroyed::DEFAULT_PAGE =
                    Box::leak(page_box) as *const crate::storage::Page<crate::component::Destroyed>;
                self::__decs_component_defaults_Destroyed::DEFAULT_CHUNK = default_chunk_ptr;
            }
        }
    }

    fn schedule_cleanup_system(world: &mut World) {
        let sys =
            TemporaryComponentCleanupSystem::<Destroyed, crate::world::DestroyGroup>::new(&*world);
        world.scheduler_mut().add_system(sys);
    }
}
