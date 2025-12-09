pub use core::ffi::{c_float, c_int, c_uint};
use core::num::NonZeroU32;

pub type GLenum = c_uint;
pub type GLbitfield = c_uint;
pub type GLint = c_int;
pub type GLuint = c_uint;
pub type GLintptr = isize;
pub type GLsizei = c_int;
pub type GLsizeiptr = isize;
pub type GLfloat = c_float;
pub type GLclampf = c_float;

macro_rules! parse_gl_names {
    ( [ $($gl_name:literal),* $(,)? ] ) => {
        [$($gl_name,)*]
    };
    ($gl_name:literal) => {
        [$gl_name]
    };
}

macro_rules! define_gl_api_mod {
    // Functions
    (
        _fn_mod_list [$( $fn_mods:ident, )*];
        #[gl = $gl_names:tt]
        pub unsafe fn $fn_name:ident (
            $( $arg_name:ident : $arg_type:ty ),*
            $(,)?
        ) $( -> $ret_type:ty )?;
        $( $rest:tt )*
    ) => {
        // Wrapper function
        pub unsafe fn $fn_name($($arg_name : $arg_type,)*) $( -> $ret_type )? {
            unsafe {
                ($fn_name::get())($($arg_name,)*)
            }
        }
        // Module
        mod $fn_name {
            #[allow(unused)]
            use super::*;

            pub type FnType = unsafe extern "system" fn($($arg_type,)*) $( -> $ret_type )?;

            pub fn get() -> FnType {
                unsafe {
                    let raw_ptr = PTR.load(::core::sync::atomic::Ordering::Relaxed);
                    ::core::mem::transmute::<*mut (), FnType>(raw_ptr)
                }
            }

            pub unsafe fn load_with(mut load_func: impl FnMut(&'static str) -> *const ()) {
                // Try each of the OpenGL name alternatives until a valid function pointer is found.
                for gl_fn_name in parse_gl_names!($gl_names) {
                    let fn_ptr = load_func(gl_fn_name);
                    if !fn_ptr.is_null() {
                        PTR.store(fn_ptr as *mut (), ::core::sync::atomic::Ordering::Relaxed);
                        return;
                    }
                }
                // If a valid function pointer wasn't found, then just load the dummy function.
                PTR.store(dummy_fn as *mut (), ::core::sync::atomic::Ordering::Relaxed);
            }

            static PTR: ::core::sync::atomic::AtomicPtr<()>
                = ::core::sync::atomic::AtomicPtr::new(dummy_fn as *mut ());

            #[allow(unused)]
            unsafe fn dummy_fn($($arg_name : $arg_type,)*) $( -> $ret_type )? {
                ::core::panic!("OpenGL function \"{}\" not loaded", stringify!($fn_name))
            }
        }
        // Add the function module to the list, parse the rest of the tokens.
        define_gl_api_mod!(
            _fn_mod_list [$( $fn_mods, )* $fn_name,];
            $( $rest )*
        );
    };
    // Enums
    (
        _fn_mod_list [$( $fn_mod:ident, )*];
        pub enum $enum_name:ident {
            $($variant_name:ident = $value:literal),*
            $(,)?
        }
        $( $rest:tt )*
    ) => {
        #[repr(i32)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $enum_name {
            $($variant_name = $value,)*
        }
        // Parse the rest of the tokens.
        define_gl_api_mod!(
            _fn_mod_list [$( $fn_mod, )*];
            $( $rest )*
        );
    };
    // Bitflags
    (
        _fn_mod_list [$( $fn_mod:ident, )*];
        pub bitfield $bitflags_name:ident {
            $(const $field_name:ident = $value:literal;)*
        }
        $( $rest:tt )*
    ) => {
        ::bitflags::bitflags! {
            #[repr(transparent)]
            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            pub struct $bitflags_name: GLbitfield {
                $(const $field_name = $value;)*
            }
        }
        // Parse the rest of the tokens.
        define_gl_api_mod!(
            _fn_mod_list [$( $fn_mod, )*];
            $( $rest )*
        );
    };
    // If we're done parsing tokens, then we're just left with a function module list.
    // Now we can create the main load function, to load all of the OpenGL pointers.
    (
        _fn_mod_list [$( $fn_mod_name:ident, )*];
    ) => {
        pub(super) unsafe fn load_mod_with(mut load_func: impl FnMut(&'static str) -> *const ()) {
            unsafe {
                $(
                    $fn_mod_name::load_with(&mut load_func);
                )*
            };
        }
    };
    // Main pattern, initialises an empty function module list and parses the rest of the tokens.
    ( $( $rest:tt )* ) => {
        define_gl_api_mod!(
            _fn_mod_list [];
            $( $rest )*
        );
    };
}

pub unsafe fn load_with(mut load_func: impl FnMut(&'static str) -> *const ()) {
    #[inline(never)]
    unsafe fn load_with_dyn(load_func: &mut dyn FnMut(&'static str) -> *const ()) {
        unsafe {
            main_state::load_mod_with(&mut *load_func);
            array::load_mod_with(&mut *load_func);
            buffer::load_mod_with(&mut *load_func);
            client_state::load_mod_with(&mut *load_func);
            display_list::load_mod_with(&mut *load_func);
            fragment::load_mod_with(&mut *load_func);
            framebuffer::load_mod_with(&mut *load_func);
            matrix::load_mod_with(&mut *load_func);
            texture::load_mod_with(&mut *load_func);
            viewport::load_mod_with(&mut *load_func);
        }
    }
    unsafe {
        load_with_dyn(&mut load_func);
    };
}

#[allow(unused)]
pub use main_state::*;
mod main_state {
    define_gl_api_mod! {
        pub enum EnableComponent {
            FaceCulling = 0x0B44,
            DepthTest = 0x0B71,
            AlphaTesting = 0x0BC0,
            Texture2D = 0x0DE1,
            ScissorTest = 0x0C11,
            Blending = 0x0BE2,
        }

        #[gl = "glEnable"]
        pub unsafe fn enable(component: EnableComponent);

        #[gl = "glDisable"]
        pub unsafe fn disable(component: EnableComponent);

        #[gl = "glFlush"]
        pub unsafe fn flush();

        #[gl = "glFinish"]
        pub unsafe fn finish();

        // Misc.

        pub enum ShapeMode {
            Points = 0x0000,
            Lines = 0x0001,
            LineLoop = 0x0002,
            LineStrip = 0x0003,
            Triangles = 0x0004,
            TriangleStrip = 0x0005,
            TriangleFan = 0x0006,
            Quads = 0x0007,
            QuadStrip = 0x0008,
            Polygon = 0x0009,
        }
    }
}

pub mod array {
    use super::*;

    define_gl_api_mod! {
        pub enum ColorPointerType {
            I8 = 0x1400,
            U8 = 0x1401,
            I16 = 0x1402,
            U16 = 0x1403,
            I32 = 0x1404,
            U32 = 0x1405,
            F32 = 0x1406,
            F64 = 0x140A,
        }

        #[gl = "glColorPointer"]
        pub unsafe fn color_pointer(
            size: GLint,
            color_type: ColorPointerType,
            stride: GLsizei,
            ptr: usize,
        );

        pub enum ColorIndexPointerType {
            I16 = 0x1402,
            I32 = 0x1404,
            F32 = 0x1406,
            F64 = 0x140A,
        }

        #[gl = "glIndexPointer"]
        pub unsafe fn color_index_pointer(
            index_type: ColorIndexPointerType,
            stride: GLsizei,
            ptr: usize,
        );

        pub enum TextureCoordPointerType {
            I16 = 0x1402,
            I32 = 0x1404,
            F32 = 0x1406,
            F64 = 0x140A,
        }

        #[gl = "glTexCoordPointer"]
        pub unsafe fn texture_coord_pointer(
            size: GLint,
            tex_coord_type: TextureCoordPointerType,
            stride: GLsizei,
            ptr: usize,
        );

        pub enum VertexPointerType {
            I16 = 0x1402,
            I32 = 0x1404,
            F32 = 0x1406,
            F64 = 0x140A,
        }

        #[gl = "glVertexPointer"]
        pub unsafe fn vertex_pointer(
            size: GLint,
            vertex_type: VertexPointerType,
            stride: GLsizei,
            ptr: usize,
        );

        #[gl = "glDrawArrays"]
        pub unsafe fn draw(mode: ShapeMode, first: GLint, element_count: GLsizei);

        pub enum IndexType {
            U8 = 0x1401,
            U16 = 0x1403,
            U32 = 0x1405,
        }

        #[gl = "glDrawElements"]
        pub unsafe fn draw_elements(
            mode: ShapeMode,
            indices_count: GLsizei,
            index_type: IndexType,
            indices_ptr: usize,
        );
    }
}

pub mod buffer {
    use super::*;

    pub type BufferHandle = NonZeroU32;

    pub unsafe fn gen_buffers<const N: usize>() -> [BufferHandle; N] {
        let mut raw_buffers: [GLuint; N] = [0; N];
        unsafe {
            gen_buffers_raw(N as GLsizei, raw_buffers.as_mut_ptr());
        }
        debug_assert!(
            raw_buffers.iter().all(|&buf| buf != 0),
            "OpenGL gen buffers failed - buffers = {raw_buffers:?}",
        );
        raw_buffers.map(|handle| NonZeroU32::new(handle).unwrap())
    }

    define_gl_api_mod! {
        #[gl = ["glGenBuffers", "glGenBuffersARB"]]
        pub unsafe fn gen_buffers_raw(num_buffers: GLsizei, out_buffers: *mut GLuint);

        #[gl = ["glDeleteBuffers", "glDeleteBuffersARB"]]
        pub unsafe fn delete_buffers_raw(num_buffers: GLsizei, buffers: *const BufferHandle);

        pub enum BufferType {
            ArrayBuffer = 0x8892,
        }

        #[gl = ["glBindBuffer", "glBindBufferARB"]]
        pub unsafe fn bind(buffer_type: BufferType, buffer: Option<BufferHandle>);

        pub enum DataUsageHint {
            StaticDraw = 0x88E4,
            DynamicDraw = 0x88E8,
        }

        #[gl = ["glBufferData", "glBufferDataARB"]]
        pub unsafe fn set_current_buffer_data_raw(
            buffer_type: BufferType,
            num_bytes: GLsizei,
            data: *const (),
            usage_hint: DataUsageHint,
        );

        #[gl = ["glBufferSubData", "glBufferSubDataARB"]]
        pub unsafe fn update_buffer_section_data_raw(
            buffer_type: BufferType,
            byte_offset: GLintptr,
            byte_size: GLsizeiptr,
            data: *const (),
        );
    }

    pub mod batch_collected {
        use super::*;
        use std::sync::Mutex;

        static POOL: Mutex<Vec<BufferHandle>> = Mutex::new(Vec::new());

        pub unsafe fn drain_pool() {
            unsafe {
                let mut pool_lock = POOL.lock().unwrap();
                let pool_slice = pool_lock.as_slice();
                delete_buffers_raw(pool_slice.len().try_into().unwrap(), pool_slice.as_ptr());
                pool_lock.clear();
            }
        }

        #[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(transparent)]
        pub struct Buffer(BufferHandle);

        impl Buffer {
            pub unsafe fn make_array<const N: usize>() -> [Self; N] {
                unsafe { gen_buffers().map(|handle| Self::from_handle(handle)) }
            }

            /// # SAFETY
            ///
            /// `handle` must not be used after calling this.
            pub unsafe fn from_handle(handle: BufferHandle) -> Self {
                Self(handle)
            }

            /// Gets the raw [`BufferHandle`] backing this buffer.
            ///
            /// # SAFETY
            ///
            /// The returned handle must not be used to delete the buffer.
            pub unsafe fn as_raw(&self) -> BufferHandle {
                self.0
            }

            pub unsafe fn bind(&self, buffer_type: BufferType) {
                unsafe { bind(buffer_type, Some(self.as_raw())) }
            }
        }

        impl Drop for Buffer {
            fn drop(&mut self) {
                POOL.lock().unwrap().push(self.0);
            }
        }
    }
}

pub mod client_state {
    define_gl_api_mod! {
        pub enum ClientArrayType {
            VertexArray = 0x8074,
            NormalArray = 0x8075,
            ColorArray = 0x8076,
            IndexArray = 0x8077,
            TextureCoordArray = 0x8078,
            EdgeFlagArray = 0x8079,
        }

        #[gl = "glEnableClientState"]
        pub unsafe fn enable(component: ClientArrayType);

        #[gl = "glDisableClientState"]
        pub unsafe fn disable(component: ClientArrayType);
    }
}

pub mod display_list {
    use super::*;

    pub type ListHandleBase = NonZeroU32;

    pub unsafe fn gen_lists<const N: usize>() -> [ListHandleBase; N] {
        assert!(N > 0);
        let list_base = unsafe { gen_lists_raw(N as GLsizei) };
        match list_base {
            None => panic!("OpenGL gen lists failed"),
            Some(base) => core::array::from_fn(|i| base.checked_add(i as u32).unwrap()),
        }
    }

    pub unsafe fn call_multiple(handles: &[ListHandleBase]) {
        unsafe {
            call_multiple_raw(
                handles.len().try_into().unwrap(),
                ListOffsetType::U32,
                handles.as_ptr() as *const (),
            );
        }
    }

    define_gl_api_mod! {
        #[gl = "glGenLists"]
        pub unsafe fn gen_lists_raw(num_lists: GLsizei) -> Option<ListHandleBase>;

        #[gl = "glDeleteLists"]
        pub unsafe fn delete_lists(start_list: ListHandleBase, num_lists: GLsizei);

        #[gl = "glCallList"]
        pub unsafe fn call(list: ListHandleBase);

        pub enum ListOffsetType {
            I8 = 0x1400,
            U8 = 0x1401,
            I16 = 0x1402,
            U16 = 0x1403,
            I32 = 0x1404,
            U32 = 0x1405,
            F32 = 0x1406,
            U16BE = 0x1407,
            U24BE = 0x1408,
            U32BE = 0x1409,
        }

        #[gl = "glCallLists"]
        pub unsafe fn call_multiple_raw(
            num_lists: GLsizei,
            offset_type: ListOffsetType,
            offsets_ptr: *const (),
        );

        pub enum RecordMode {
            Compile = 0x1300,
            CompileAndExecute = 0x1301,
        }

        #[gl = "glNewList"]
        pub unsafe fn start_recording(list: ListHandleBase, mode: RecordMode);

        #[gl = "glEndList"]
        pub unsafe fn stop_recording();
    }

    pub mod batch_collected {
        use super::*;
        use std::sync::Mutex;

        static POOL: Mutex<Vec<ListHandleBase>> = Mutex::new(Vec::new());

        pub unsafe fn drain_pool() {
            unsafe {
                let mut pool_lock = POOL.lock().unwrap();
                for list_handle in pool_lock.drain(..) {
                    delete_lists(list_handle, 1);
                }
            }
        }

        #[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(transparent)]
        pub struct DisplayList(ListHandleBase);

        impl DisplayList {
            pub unsafe fn make_array<const N: usize>() -> [Self; N] {
                unsafe { gen_lists().map(|handle| Self::from_handle(handle)) }
            }

            /// # SAFETY
            ///
            /// `handle` must not be used after calling this.
            pub unsafe fn from_handle(handle: ListHandleBase) -> Self {
                Self(handle)
            }

            /// Gets the raw [`BufferHandle`] backing this buffer.
            ///
            /// # SAFETY
            ///
            /// The returned handle must not be used to delete the buffer.
            pub unsafe fn as_raw(&self) -> ListHandleBase {
                self.0
            }

            pub unsafe fn record_with(&self, mode: RecordMode, record_fn: impl FnOnce()) {
                unsafe {
                    start_recording(self.as_raw(), mode);
                    record_fn();
                    stop_recording();
                }
            }
        }

        impl Drop for DisplayList {
            fn drop(&mut self) {
                POOL.lock().unwrap().push(self.0);
            }
        }
    }
}

pub mod fragment {
    use super::*;

    define_gl_api_mod! {
        #[gl = "glScissor"]
        pub unsafe fn set_scissor(left: GLint, bottom: GLint, width: GLsizei, height: GLsizei);

        pub enum BlendEquationFunc {
            Add = 0x8006,
        }

        #[gl = "glBlendEquation"]
        pub unsafe fn set_blend_equation(equation: BlendEquationFunc);

        pub enum BlendFactor {
            Zero = 0,
            One = 1,
            SrcAlpha = 0x0302,
            OneMinusSrcAlpha = 0x0303,
            DstAlpha = 0x0304,
            OneMinusDstAlpha = 0x0305,
        }

        #[gl = "glBlendFunc"]
        pub unsafe fn set_blend_function(src: BlendFactor, dst: BlendFactor);

        pub enum AlphaTestFunc {
            Never = 0x0200,
            Less = 0x0201,
            Equal = 0x0202,
            LessThanOrEqual = 0x0203,
            Greater = 0x0204,
            NotEqual = 0x0205,
            GreaterThanOrEqual = 0x0206,
            Always = 0x0207,
        }

        #[gl = "glAlphaFunc"]
        pub unsafe fn set_alpha_test_function(func: AlphaTestFunc, reference: GLclampf);
    }
}

pub mod framebuffer {
    use super::*;

    define_gl_api_mod! {
        pub bitfield ClearBufferBits {
            const DEPTH     = 0x00000100;
            const STENCIL   = 0x00000200;
            const ACCUM     = 0x00000400;
            const COLOR     = 0x00004000;
        }

        #[gl = "glClear"]
        pub unsafe fn clear(buffer_bits: ClearBufferBits);

        #[gl = "glClearColor"]
        pub unsafe fn clear_color(r: GLclampf, g: GLclampf, b: GLclampf, a: GLclampf);
    }
}

pub mod matrix {
    use super::*;

    define_gl_api_mod! {
        pub enum MatrixMode {
            ModelView = 0x1700,
            Projection = 0x1701,
            Texture = 0x1702,
        }

        #[gl = "glMatrixMode"]
        pub unsafe fn switch_mode(mode: MatrixMode);

        #[gl = "glLoadMatrixf"]
        pub unsafe fn load_f32_matrix(matrix: &[[GLfloat; 4]; 4]);

        #[gl = "glLoadIdentity"]
        pub unsafe fn load_identity();
    }
}

pub mod texture {
    use super::*;

    pub type TextureHandle = NonZeroU32;

    pub unsafe fn gen_textures<const N: usize>() -> [TextureHandle; N] {
        let mut raw_textures: [GLuint; N] = [0; N];
        unsafe {
            gen_textures_raw(N as GLsizei, raw_textures.as_mut_ptr());
        }
        assert!(
            raw_textures.iter().all(|&buf| buf != 0),
            "OpenGL gen textures failed - textures = {raw_textures:?}",
        );
        raw_textures.map(|handle| NonZeroU32::new(handle).unwrap())
    }

    pub unsafe fn set_env_mode(target: TexEnvTarget, mode: TexEnvMode) {
        unsafe { set_env_raw_i32(target, TexEnvParam::Mode, mode as c_int) }
    }

    pub unsafe fn set_mag_filter(target: TexTarget, mode: TexFilterMode) {
        unsafe { set_tex_param_raw_i32(target, TexParam::MagFilter, mode as c_int) }
    }

    pub unsafe fn set_min_filter(target: TexTarget, mode: TexFilterMode) {
        unsafe { set_tex_param_raw_i32(target, TexParam::MinFilter, mode as c_int) }
    }

    pub unsafe fn set_wrap_s(target: TexTarget, mode: TexWrapMode) {
        unsafe { set_tex_param_raw_i32(target, TexParam::WrapS, mode as c_int) }
    }

    pub unsafe fn set_wrap_t(target: TexTarget, mode: TexWrapMode) {
        unsafe { set_tex_param_raw_i32(target, TexParam::WrapT, mode as c_int) }
    }

    define_gl_api_mod! {
        #[gl = "glGenTextures"]
        pub unsafe fn gen_textures_raw(num_buffers: GLsizei, out_buffers: *mut GLuint);

        #[gl = "glDeleteTextures"]
        pub unsafe fn delete_textures_raw(num_buffers: GLsizei, buffers: *const TextureHandle);

        pub enum TexTarget {
            Texture2D = 0x0DE1,
        }

        #[gl = "glBindTexture"]
        pub unsafe fn bind(target: TexTarget, texture: Option<TextureHandle>);

        pub enum TextureDataType {
            I8 = 0x1400,
            U8 = 0x1401,
            I16 = 0x1402,
            U16 = 0x1403,
            I32 = 0x1404,
            U32 = 0x1405,
            F32 = 0x1406,
        }

        pub enum TextureInternalFormat {
            // Symbolic formats
            Alpha = 0x1906,
            Luminance = 0x1909,
            LuminanceAlpha = 0x190A,
            Intensity = 0x8049,
            Rgb = 0x1907,
            Rgba = 0x1908,
            // Sized formats
            R3G3B2 = 0x2A10,
            Rgb4 = 0x804F,
            Rgb5 = 0x8050,
            Rgb8 = 0x8051,
            Rgba4 = 0x8056,
            Rgb5A1 = 0x8057,
            Rgba8 = 0x8058,
            Rgb10A2 = 0x8059,
        }

        pub enum Texture2dTarget {
            Texture = 0x0DE1,
        }

        pub enum Texture2dFormat {
            ColorIndex = 0x1900,
            R = 0x1903,
            G = 0x1904,
            B = 0x1905,
            A = 0x1906,
            Rgb = 0x1907,
            Rgba = 0x1908,
            Bgr = 0x80E0,
            Bgra = 0x80E1,
            Luminance = 0x1909,
            LuminanceAlpha = 0x190A,
        }

        #[gl = "glTexImage2D"]
        pub unsafe fn set_image_2d(
            target: Texture2dTarget,
            level: GLint,
            internal_format: TextureInternalFormat,
            width: GLsizei,
            height: GLsizei,
            border: GLint,
            format: Texture2dFormat,
            data_type: TextureDataType,
            pixels: *const (),
        );

        pub enum TexEnvTarget {
            TextureEnv = 0x2300,
        }

        pub enum TexEnvParam {
            Mode = 0x2200,
        }

        pub enum TexEnvMode {
            Replace = 0x1E01,
            Modulate = 0x2100,
            Decal = 0x2101,
        }

        #[gl = "glTexEnvi"]
        pub unsafe fn set_env_raw_i32(target: TexEnvTarget, param: TexEnvParam, value: GLint);

        #[gl = "glTexEnvf"]
        pub unsafe fn set_env_raw_f32(target: TexEnvTarget, param: TexEnvParam, value: GLfloat);

        pub enum TexParam {
            MagFilter = 0x2800,
            MinFilter = 0x2801,
            WrapS = 0x2802,
            WrapT = 0x2803,
        }

        pub enum TexFilterMode {
            Nearest = 0x2600,
            Linear = 0x2601,
        }

        pub enum TexWrapMode {
            Clamp = 0x2900,
            Repeat = 0x2901,
            ClampToEdge = 0x812F,
            MirroredRepeat = 0x8370,
        }

        #[gl = "glTexParameteri"]
        pub unsafe fn set_tex_param_raw_i32(target: TexTarget, param: TexParam, value: GLint);

        pub enum PixelStoreParam {
            UnpackAlignment = 0x0CF5,
        }

        #[gl = "glPixelStorei"]
        pub unsafe fn set_pixel_store_i32_raw(param: PixelStoreParam, value: GLint);
    }

    pub mod batch_collected {
        use super::*;
        use std::sync::Mutex;

        static POOL: Mutex<Vec<TextureHandle>> = Mutex::new(Vec::new());

        pub unsafe fn drain_pool() {
            unsafe {
                let mut pool_lock = POOL.lock().unwrap();
                let pool_slice = pool_lock.as_slice();
                delete_textures_raw(pool_slice.len().try_into().unwrap(), pool_slice.as_ptr());
                pool_lock.clear();
            }
        }

        #[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(transparent)]
        pub struct Texture(TextureHandle);

        impl Texture {
            pub unsafe fn make_array<const N: usize>() -> [Self; N] {
                unsafe { gen_textures().map(|handle| Self::from_handle(handle)) }
            }

            /// # SAFETY
            ///
            /// `handle` must not be used after calling this.
            pub unsafe fn from_handle(handle: TextureHandle) -> Self {
                Self(handle)
            }

            /// Gets the raw [`TextureHandle`] backing this texture.
            ///
            /// # SAFETY
            ///
            /// The returned handle must not be used to delete the texture.
            pub unsafe fn as_raw(&self) -> TextureHandle {
                self.0
            }

            pub unsafe fn bind(&self, target: TexTarget) {
                unsafe { bind(target, Some(self.as_raw())) }
            }
        }

        impl Drop for Texture {
            fn drop(&mut self) {
                POOL.lock().unwrap().push(self.0);
            }
        }
    }
}

pub mod viewport {
    use super::*;

    define_gl_api_mod! {
        #[gl = "glViewport"]
        pub unsafe fn set(x: GLint, y: GLint, width: GLsizei, height: GLsizei);
    }
}
