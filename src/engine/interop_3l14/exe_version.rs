use std::{error::Error, ffi::OsStr, fmt::{Debug, Display, Formatter}, os::windows::ffi::OsStrExt};

use nab_3l14::utils::alloc_slice::alloc_slice_uninit;
use windows::{Win32::{Foundation::{ERROR_FILE_NOT_FOUND, ERROR_RESOURCE_DATA_NOT_FOUND, GetLastError}, Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW}}, core::PCWSTR};

#[derive(Default)]
pub struct Version
{
    pub major: u16,
    pub minor: u16,
    pub build: u16,
    pub revision: u16,
}
impl Version
{
    pub fn as_bytes(&self) -> [u8; 8]
    {
        let mut bytes = [0u8; 8];
        bytes[0..2].copy_from_slice(&self.major.to_le_bytes());
        bytes[2..4].copy_from_slice(&self.minor.to_le_bytes());
        bytes[4..6].copy_from_slice(&self.build.to_le_bytes());
        bytes[6..8].copy_from_slice(&self.revision.to_le_bytes());
        return bytes;
    }
}

#[derive(Debug)]
pub enum GetVersionError
{
    FileNotFound,
    NoVersionInfo,
    Win32Error(u32),
}
impl Display for GetVersionError
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { Debug::fmt(self, f) }
}
impl Error for GetVersionError {}

fn to_widestr(s: &OsStr) -> Box<[u16]>
{
    s.encode_wide().chain(Some(0)).collect()
}

pub fn get_exe_version(bin_path: impl AsRef<OsStr>) -> Result<Version, GetVersionError>
{
    let wide = to_widestr(bin_path.as_ref());

    let mut handle = 0u32;
    let size = unsafe { GetFileVersionInfoSizeW(PCWSTR(wide.as_ptr()), Some(&mut handle)) };
    if size == 0
    {
        return match unsafe { GetLastError() }
        {
            ERROR_FILE_NOT_FOUND => Err(GetVersionError::FileNotFound),
            ERROR_RESOURCE_DATA_NOT_FOUND => Err(GetVersionError::NoVersionInfo),
            err => Err(GetVersionError::Win32Error(err.0)),
        };
    }


    let buffer = unsafe
    {
        let mut buffer = alloc_slice_uninit(size as usize);
        if let Err(e) = GetFileVersionInfoW(
                PCWSTR(wide.as_ptr()),
                None,
                size,
                buffer.as_mut_ptr() as *mut _)
        {
            return Err(GetVersionError::Win32Error(e.code().0 as u32));
        }

        buffer
    };

    let mut ffi: *mut VS_FIXEDFILEINFO = std::ptr::null_mut();
    let mut len = 0u32;

    let ok = unsafe
    {
        const VER_ROOT: [u16; 2] = [b'\\' as u16, 0];
        VerQueryValueW(
            buffer.as_ptr() as *const _,
            PCWSTR(VER_ROOT.as_ptr()),
            &mut ffi as *mut _ as *mut _,
            &mut len,
        )
        .as_bool()
    };

    if !ok || ffi.is_null() || len < std::mem::size_of::<VS_FIXEDFILEINFO>() as u32
    {
        return Err(GetVersionError::NoVersionInfo);
    }

    let ffi = unsafe { &*ffi };
    Ok(Version {
        major: (ffi.dwFileVersionMS >> 16) as u16,
        minor: (ffi.dwFileVersionMS & 0xFFFF) as u16,
        build: (ffi.dwFileVersionLS >> 16) as u16,
        revision: (ffi.dwFileVersionLS & 0xFFFF) as u16,
    })
}
