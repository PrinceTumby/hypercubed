#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        use core::arch::x86_64::*;
    } else {
        use core::arch::x86::*;
    }
}

const ROUND_NEAREST_INT_NO_EXC: i32 = _MM_FROUND_TO_NEAREST_INT | _MM_FROUND_NO_EXC;

// u8x16

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct u8x16(pub __m128i);

impl u8x16 {
    #[inline(always)]
    pub const fn from_array(values: [u8; 16]) -> Self {
        unsafe { Self(core::mem::transmute::<[u8; 16], __m128i>(values)) }
    }

    #[inline(always)]
    pub fn splat(value: u8) -> Self {
        Self(unsafe { _mm_set1_epi8(value as i8) })
    }

    #[inline(always)]
    pub fn saturating_add(self, rhs: Self) -> Self {
        Self(unsafe { _mm_adds_epu8(self.0, rhs.0) })
    }

    #[inline(always)]
    pub fn saturating_sub(self, rhs: Self) -> Self {
        Self(unsafe { _mm_subs_epu8(self.0, rhs.0) })
    }

    #[inline(always)]
    pub fn simd_lt(self, rhs: Self) -> mask16 {
        mask16(unsafe { _mm_cmp_epu8_mask::<_MM_CMPINT_LT>(self.0, rhs.0) })
    }

    #[inline(always)]
    pub fn simd_le(self, rhs: Self) -> mask16 {
        mask16(unsafe { _mm_cmp_epu8_mask::<_MM_CMPINT_LE>(self.0, rhs.0) })
    }

    #[inline(always)]
    pub fn simd_gt(self, rhs: Self) -> mask16 {
        mask16(unsafe { _mm_cmp_epu8_mask::<_MM_CMPINT_LT>(rhs.0, self.0) })
    }

    #[inline(always)]
    pub fn simd_ge(self, rhs: Self) -> mask16 {
        mask16(unsafe { _mm_cmp_epu8_mask::<_MM_CMPINT_LE>(rhs.0, self.0) })
    }

    #[inline(always)]
    pub fn cast_u32(self) -> u32x16 {
        u32x16(unsafe { _mm512_cvtepu8_epi32(self.0) })
    }
}

impl core::ops::Add for u8x16 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        Self(unsafe { _mm_add_epi8(self.0, rhs.0) })
    }
}

impl core::ops::Sub for u8x16 {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
        Self(unsafe { _mm_sub_epi8(self.0, rhs.0) })
    }
}

// u32x16

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct u32x16(pub __m512i);

impl u32x16 {
    #[inline(always)]
    pub const fn from_array(values: [u32; 16]) -> Self {
        unsafe { Self(core::mem::transmute::<[u32; 16], __m512i>(values)) }
    }

    #[inline(always)]
    pub const fn to_array(self) -> [u32; 16] {
        unsafe { core::mem::transmute(self) }
    }

    #[inline(always)]
    pub fn splat(value: u32) -> Self {
        Self(unsafe { _mm512_set1_epi32(value as i32) })
    }

    /// Splits `self` into 4 groups of `u32x4` vectors, then converts each vector to native-endian
    /// bytes.
    #[inline(always)]
    pub fn to_ne_16_byte_groups(self) -> [[u8; 16]; 4] {
        unsafe { core::mem::transmute(self) }
    }

    #[inline(always)]
    pub unsafe fn gather_from_offsets(base: *const u32, offsets: Self) -> Self {
        Self(unsafe { _mm512_i32gather_epi32(offsets.0, base.cast::<i32>(), 4) })
    }

    /// Elements not loaded due to being masked out, will be set to zero.
    #[inline(always)]
    pub unsafe fn gather_from_offsets_masked(
        base: *const u32,
        offsets: Self,
        enable: mask16,
    ) -> Self {
        Self(unsafe {
            _mm512_mask_i32gather_epi32(
                _mm512_setzero_epi32(),
                enable.0,
                offsets.0,
                base.cast::<i32>(),
                4,
            )
        })
    }

    #[inline(always)]
    pub fn store_masked(self, ptr: &mut Self, enable: mask16) {
        unsafe {
            _mm512_mask_store_epi32((ptr as *mut Self).cast::<i32>(), enable.0, self.0);
        }
    }

    #[inline(always)]
    pub fn cast_u8(self) -> u8x16 {
        u8x16(unsafe { _mm512_cvtepi32_epi8(self.0) })
    }

    #[inline(always)]
    pub fn cast_f32(self) -> f32x16 {
        f32x16(unsafe { _mm512_cvtepi32_ps(self.0) })
    }
}

impl core::ops::Add for u32x16 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        Self(unsafe { _mm512_add_epi32(self.0, rhs.0) })
    }
}

impl core::ops::Sub for u32x16 {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
        Self(unsafe { _mm512_sub_epi32(self.0, rhs.0) })
    }
}

impl core::ops::Mul for u32x16 {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
        Self(unsafe { _mm512_mullo_epi32(self.0, rhs.0) })
    }
}

impl core::ops::BitAnd for u32x16 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Self) -> Self {
        Self(unsafe { _mm512_and_epi32(self.0, rhs.0) })
    }
}

impl core::ops::BitOr for u32x16 {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self {
        Self(unsafe { _mm512_or_epi32(self.0, rhs.0) })
    }
}

impl core::ops::Shl for u32x16 {
    type Output = Self;

    #[inline(always)]
    fn shl(self, rhs: Self) -> Self {
        Self(unsafe { _mm512_shrdv_epi32(self.0, _mm512_setzero_epi32(), rhs.0) })
    }
}

impl core::ops::BitAnd<u32> for u32x16 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: u32) -> Self {
        Self(unsafe { _mm512_and_epi32(self.0, Self::splat(rhs).0) })
    }
}

impl core::ops::BitOr<u32> for u32x16 {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: u32) -> Self {
        Self(unsafe { _mm512_or_epi32(self.0, Self::splat(rhs).0) })
    }
}

impl core::ops::Shl<u32> for u32x16 {
    type Output = Self;

    #[inline(always)]
    fn shl(self, rhs: u32) -> Self {
        Self(unsafe { _mm512_sll_epi32(self.0, _mm_set_epi32(0, 0, 0, rhs as i32)) })
    }
}

impl core::ops::Shr<u32> for u32x16 {
    type Output = Self;

    #[inline(always)]
    fn shr(self, rhs: u32) -> Self {
        Self(unsafe { _mm512_srl_epi32(self.0, _mm_set_epi32(0, 0, 0, rhs as i32)) })
    }
}

impl core::ops::Index<usize> for u32x16 {
    type Output = u32;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < 16);
        unsafe { &*((self as *const Self as *const u32).add(index)) }
    }
}

impl core::ops::IndexMut<usize> for u32x16 {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < 16);
        unsafe { &mut *((self as *mut Self as *mut u32).add(index)) }
    }
}

// f32x4

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct f32x4(pub __m128);

impl f32x4 {
    #[inline(always)]
    pub const fn from_array(values: [f32; 4]) -> Self {
        unsafe { Self(core::mem::transmute::<[f32; 4], __m128>(values)) }
    }

    #[inline(always)]
    pub const fn to_array(self) -> [f32; 4] {
        unsafe { core::mem::transmute(self) }
    }

    #[inline(always)]
    pub fn splat(value: f32) -> Self {
        Self(unsafe { _mm_set1_ps(value) })
    }

    /// Calculates `(self * a) + b`.
    #[inline(always)]
    pub fn mul_add(self, a: Self, b: Self) -> Self {
        Self(unsafe { _mm_fmadd_ps(self.0, a.0, b.0) })
    }

    /// Calculates `b - (self * a)`.
    #[inline(always)]
    pub fn mul_neg_add(self, a: Self, b: Self) -> Self {
        Self(unsafe { _mm_fnmadd_ps(self.0, a.0, b.0) })
    }
}

impl core::ops::Add for f32x4 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        Self(unsafe { _mm_add_ps(self.0, rhs.0) })
    }
}

impl core::ops::AddAssign for f32x4 {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        *self = Self(unsafe { _mm_add_ps(self.0, rhs.0) });
    }
}

impl core::ops::Sub for f32x4 {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
        Self(unsafe { _mm_sub_ps(self.0, rhs.0) })
    }
}

impl core::ops::Mul for f32x4 {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
        Self(unsafe { _mm_mul_ps(self.0, rhs.0) })
    }
}

impl core::ops::Div for f32x4 {
    type Output = Self;

    #[inline(always)]
    fn div(self, rhs: Self) -> Self {
        Self(unsafe { _mm_div_ps(self.0, rhs.0) })
    }
}

// f32x16

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct f32x16(pub __m512);

impl f32x16 {
    #[inline(always)]
    pub const fn from_array(values: [f32; 16]) -> Self {
        unsafe { Self(core::mem::transmute::<[f32; 16], __m512>(values)) }
    }

    #[inline(always)]
    pub const fn to_array(self) -> [f32; 16] {
        unsafe { core::mem::transmute(self) }
    }

    #[inline(always)]
    pub fn splat(value: f32) -> Self {
        Self(unsafe { _mm512_set1_ps(value) })
    }

    pub fn load_zmasked(ptr: &Self, enable: mask16) -> Self {
        Self(unsafe { _mm512_maskz_load_ps(enable.0, (ptr as *const Self).cast::<f32>()) })
    }

    #[inline(always)]
    pub fn store_masked(self, ptr: &mut Self, enable: mask16) {
        unsafe {
            _mm512_mask_store_ps((ptr as *mut Self).cast::<f32>(), enable.0, self.0);
        }
    }

    /// Calculates `(self * a) + b`, rounded to the nearest integer.
    #[inline(always)]
    pub fn mul_sub_round(self, a: Self, b: Self) -> Self {
        unsafe {
            Self(_mm512_fmsub_round_ps::<ROUND_NEAREST_INT_NO_EXC>(
                self.0, a.0, b.0,
            ))
        }
    }

    #[inline(always)]
    pub fn clamp_all(self, min: f32, max: f32) -> Self {
        unsafe {
            Self(_mm512_min_ps(
                _mm512_max_ps(self.0, _mm512_set1_ps(min)),
                _mm512_set1_ps(max),
            ))
        }
    }

    #[inline(always)]
    pub fn abs(self) -> Self {
        unsafe { Self(_mm512_abs_ps(self.0)) }
    }

    #[inline(always)]
    pub fn simd_lt(self, rhs: Self) -> mask16 {
        mask16(unsafe { _mm512_cmp_ps_mask::<_CMP_LT_OQ>(self.0, rhs.0) })
    }

    #[inline(always)]
    pub fn simd_le(self, rhs: Self) -> mask16 {
        mask16(unsafe { _mm512_cmp_ps_mask::<_CMP_LE_OQ>(self.0, rhs.0) })
    }

    #[inline(always)]
    pub fn simd_gt(self, rhs: Self) -> mask16 {
        mask16(unsafe { _mm512_cmp_ps_mask::<_CMP_GT_OQ>(self.0, rhs.0) })
    }

    #[inline(always)]
    pub fn simd_ge(self, rhs: Self) -> mask16 {
        mask16(unsafe { _mm512_cmp_ps_mask::<_CMP_GE_OQ>(self.0, rhs.0) })
    }

    #[inline(always)]
    pub fn reduce_min(self) -> f32 {
        unsafe { _mm512_reduce_min_ps(self.0) }
    }

    #[inline(always)]
    pub fn to_nearest_unorm8(self) -> unorm8x16 {
        unsafe {
            let scaled =
                _mm512_mul_round_ps::<ROUND_NEAREST_INT_NO_EXC>(self.0, _mm512_set1_ps(255.0));
            let clamped = _mm512_min_ps(
                _mm512_max_ps(scaled, _mm512_set1_ps(0.0)),
                _mm512_set1_ps(255.0),
            );
            unorm8x16(u32x16(_mm512_cvtps_epi32(clamped)).cast_u8())
        }
    }

    #[inline(always)]
    pub fn cast_u32(self) -> u32x16 {
        u32x16(unsafe { _mm512_cvtps_epi32(self.0) })
    }
}

impl core::ops::Add for f32x16 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        Self(unsafe { _mm512_add_ps(self.0, rhs.0) })
    }
}

impl core::ops::Sub for f32x16 {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
        Self(unsafe { _mm512_sub_ps(self.0, rhs.0) })
    }
}

impl core::ops::Mul for f32x16 {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
        Self(unsafe { _mm512_mul_ps(self.0, rhs.0) })
    }
}

impl core::ops::Div for f32x16 {
    type Output = Self;

    #[inline(always)]
    fn div(self, rhs: Self) -> Self {
        Self(unsafe { _mm512_div_ps(self.0, rhs.0) })
    }
}

impl core::ops::Index<usize> for f32x16 {
    type Output = f32;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < 16);
        unsafe { &*((self as *const Self as *const f32).add(index)) }
    }
}

impl core::ops::IndexMut<usize> for f32x16 {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < 16);
        unsafe { &mut *((self as *mut Self as *mut f32).add(index)) }
    }
}

// mask32x16

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct mask16(pub __mmask16);

impl mask16 {
    #[inline(always)]
    pub fn splat(value: bool) -> Self {
        if value { Self(0xFFFF) } else { Self(0x0000) }
    }

    #[inline(always)]
    pub fn first_set(self) -> Option<usize> {
        match self.0.trailing_zeros() {
            16 => None,
            idx => Some(idx as usize),
        }
    }

    #[inline(always)]
    pub fn set(&mut self, idx: usize, value: bool) {
        self.0 = unsafe {
            _mm512_kor(
                _mm512_kandn(_cvtu32_mask16(1 << idx), self.0),
                _cvtu32_mask16((value as u32) << idx),
            )
        };
    }

    #[inline(always)]
    pub fn select_u32(self, true_values: u32x16, false_values: u32x16) -> u32x16 {
        u32x16(unsafe { _mm512_mask_mov_epi32(false_values.0, self.0, true_values.0) })
    }
}

impl core::ops::BitAnd for mask16 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Self) -> Self {
        Self(unsafe { _mm512_kand(self.0, rhs.0) })
    }
}

impl core::ops::BitAndAssign for mask16 {
    #[inline(always)]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 = unsafe { _mm512_kand(self.0, rhs.0) };
    }
}

// unorm8x16

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct unorm8x16(pub u8x16);

impl unorm8x16 {
    pub const ONES: Self = Self(u8x16::from_array([0xFF; 16]));

    #[inline(always)]
    pub fn simd_lt(self, rhs: Self) -> mask16 {
        self.0.simd_lt(rhs.0)
    }

    #[inline(always)]
    pub fn simd_le(self, rhs: Self) -> mask16 {
        self.0.simd_le(rhs.0)
    }

    #[inline(always)]
    pub fn simd_gt(self, rhs: Self) -> mask16 {
        self.0.simd_gt(rhs.0)
    }

    #[inline(always)]
    pub fn simd_ge(self, rhs: Self) -> mask16 {
        self.0.simd_ge(rhs.0)
    }
}

impl core::ops::Add for unorm8x16 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl core::ops::Sub for unorm8x16 {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl core::ops::Mul for unorm8x16 {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
        unsafe {
            let self_u16s = _mm256_cvtepu8_epi16(self.0.0);
            let rhs_u16s = _mm256_cvtepu8_epi16(rhs.0.0);
            let mut out = _mm256_mullo_epi16(self_u16s, rhs_u16s);
            out = _mm256_add_epi16(out, _mm256_set1_epi16(128));
            out = _mm256_add_epi16(out, _mm256_srli_epi16(out, 8));
            Self(u8x16(_mm256_cvtepi16_epi8(_mm256_srli_epi16(out, 8))))
        }
    }
}
