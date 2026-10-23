//! Windows OLE `IDropTarget` for Explorer file drops with accurate client points.
//!
//! Falls back is handled by the winit `HoveredFile`/`DroppedFile` path in `app.rs`.

use crate::drop_target::{is_supported_audio, DropZone};
use std::path::PathBuf;
use std::sync::Arc;

/// Events delivered from the OLE drop target onto the UI thread.
#[derive(Debug, Clone)]
pub enum OleDropEvent {
    Hover {
        zone: DropZone,
        paths: Vec<PathBuf>,
    },
    Leave,
    Drop {
        zone: DropZone,
        paths: Vec<PathBuf>,
        client_x: f64,
        client_y: f64,
    },
}

pub type OleDropCallback = Arc<dyn Fn(OleDropEvent) + Send + Sync>;

/// Hit-test layout published from the UI (logical pixels).
#[derive(Clone, Default)]
pub struct SharedDropLayout {
    inner: Arc<parking_lot::Mutex<crate::drop_target::DropLayout>>,
    scale: Arc<parking_lot::Mutex<f64>>,
}

impl SharedDropLayout {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(parking_lot::Mutex::new(Default::default())),
            scale: Arc::new(parking_lot::Mutex::new(1.0)),
        }
    }

    pub fn set_layout(&self, layout: crate::drop_target::DropLayout) {
        *self.inner.lock() = layout;
    }

    pub fn set_scale(&self, scale: f64) {
        *self.scale.lock() = scale.max(0.1);
    }

    pub fn hit_test_physical(&self, px: f64, py: f64) -> DropZone {
        let scale = *self.scale.lock();
        let layout = self.inner.lock();
        layout.hit_test(px / scale, py / scale)
    }

    pub fn layout_handle(&self) -> Arc<parking_lot::Mutex<crate::drop_target::DropLayout>> {
        Arc::clone(&self.inner)
    }
}

#[cfg(windows)]
mod ole {
    use super::*;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::ptr;
    use std::sync::atomic::{AtomicIsize, Ordering};

    // Minimal COM / OLE bindings.
    #[repr(C)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    const IID_IDROPTARGET: Guid = Guid {
        data1: 0x00000122,
        data2: 0x0000,
        data3: 0x0000,
        data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    };

    const IID_IDATAOBJECT: Guid = Guid {
        data1: 0x0000010E,
        data2: 0x0000,
        data3: 0x0000,
        data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    };

    type Hresult = i32;
    const S_OK: Hresult = 0;
    const E_NOINTERFACE: Hresult = -2147467262; // 0x80004002
    const E_NOTIMPL: Hresult = -2147467263; // 0x80004001
    const DRAGDROP_S_DROP: Hresult = 0x00040100;
    const DRAGDROP_S_CANCEL: Hresult = 0x00040101;
    const DVASPECT_CONTENT: u32 = 1;
    const TYMED_HGLOBAL: u32 = 1;
    const CF_HDROP: u16 = 15;
    const DROPEFFECT_COPY: u32 = 1;
    const DROPEFFECT_NONE: u32 = 0;

    #[repr(C)]
    struct PointL {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct FormatEtc {
        cf_format: u16,
        ptd: *mut std::ffi::c_void,
        dw_aspect: u32,
        lindex: i32,
        tymed: u32,
    }

    #[repr(C)]
    struct StgMedium {
        tymed: u32,
        h_global: isize,
        p_unk_for_release: *mut std::ffi::c_void,
    }

    #[repr(C)]
    struct IUnknownVtbl {
        query_interface: unsafe extern "system" fn(
            this: *mut IDropTarget,
            riid: *const Guid,
            ppv: *mut *mut std::ffi::c_void,
        ) -> Hresult,
        add_ref: unsafe extern "system" fn(this: *mut IDropTarget) -> u32,
        release: unsafe extern "system" fn(this: *mut IDropTarget) -> u32,
    }

    #[repr(C)]
    struct IDropTargetVtbl {
        unknown: IUnknownVtbl,
        drag_enter: unsafe extern "system" fn(
            this: *mut IDropTarget,
            p_data_obj: *mut std::ffi::c_void,
            grf_key_state: u32,
            pt: PointL,
            pdw_effect: *mut u32,
        ) -> Hresult,
        drag_over: unsafe extern "system" fn(
            this: *mut IDropTarget,
            grf_key_state: u32,
            pt: PointL,
            pdw_effect: *mut u32,
        ) -> Hresult,
        drag_leave: unsafe extern "system" fn(this: *mut IDropTarget) -> Hresult,
        drop: unsafe extern "system" fn(
            this: *mut IDropTarget,
            p_data_obj: *mut std::ffi::c_void,
            grf_key_state: u32,
            pt: PointL,
            pdw_effect: *mut u32,
        ) -> Hresult,
    }

    #[repr(C)]
    struct IDropTarget {
        vtbl: *const IDropTargetVtbl,
        ref_count: u32,
        hwnd: isize,
        layout: SharedDropLayout,
        callback: OleDropCallback,
        paths: Vec<PathBuf>,
    }

    static VTBL: IDropTargetVtbl = IDropTargetVtbl {
        unknown: IUnknownVtbl {
            query_interface,
            add_ref,
            release,
        },
        drag_enter,
        drag_over,
        drag_leave,
        drop,
    };

    extern "system" {
        fn OleInitialize(pv_reserved: *mut std::ffi::c_void) -> Hresult;
        fn RegisterDragDrop(hwnd: isize, p_drop_target: *mut IDropTarget) -> Hresult;
        fn RevokeDragDrop(hwnd: isize) -> Hresult;
        fn ScreenToClient(h_wnd: isize, lp_point: *mut PointL) -> i32;
        fn DragQueryFileW(h_drop: isize, i_file: u32, lpsz_file: *mut u16, cch: u32) -> u32;
        fn ReleaseStgMedium(pmedium: *mut StgMedium);
    }

    // IDataObject::GetData — vtable index 3 after IUnknown's 3 methods.
    type GetDataFn = unsafe extern "system" fn(
        this: *mut std::ffi::c_void,
        pformatetc: *const FormatEtc,
        pmedium: *mut StgMedium,
    ) -> Hresult;

    unsafe fn data_object_get_hdrop(p_data_obj: *mut std::ffi::c_void) -> Option<Vec<PathBuf>> {
        if p_data_obj.is_null() {
            return None;
        }
        let vtbl = *(p_data_obj as *mut *mut *mut std::ffi::c_void);
        // IUnknown: QI, AddRef, Release = 0..2; GetData = 3
        let get_data: GetDataFn = std::mem::transmute(*vtbl.add(3));
        let fmt = FormatEtc {
            cf_format: CF_HDROP,
            ptd: ptr::null_mut(),
            dw_aspect: DVASPECT_CONTENT,
            lindex: -1,
            tymed: TYMED_HGLOBAL,
        };
        let mut medium = StgMedium {
            tymed: 0,
            h_global: 0,
            p_unk_for_release: ptr::null_mut(),
        };
        if get_data(p_data_obj, &fmt, &mut medium) != S_OK {
            return None;
        }
        let paths = if medium.tymed == TYMED_HGLOBAL && medium.h_global != 0 {
            hdrop_to_paths(medium.h_global)
        } else {
            None
        };
        ReleaseStgMedium(&mut medium);
        paths
    }

    unsafe fn hdrop_to_paths(h_drop: isize) -> Option<Vec<PathBuf>> {
        let count = DragQueryFileW(h_drop, 0xFFFF_FFFF, ptr::null_mut(), 0);
        if count == 0 {
            return Some(Vec::new());
        }
        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count {
            let mut buf = vec![0u16; 512];
            let n = DragQueryFileW(h_drop, i, buf.as_mut_ptr(), buf.len() as u32);
            if n == 0 {
                continue;
            }
            buf.truncate(n as usize);
            let os = OsString::from_wide(&buf);
            out.push(PathBuf::from(os));
        }
        Some(out)
    }

    unsafe fn client_from_screen(hwnd: isize, pt: PointL) -> (f64, f64) {
        let mut p = pt;
        ScreenToClient(hwnd, &mut p);
        (p.x as f64, p.y as f64)
    }

    unsafe extern "system" fn query_interface(
        this: *mut IDropTarget,
        riid: *const Guid,
        ppv: *mut *mut std::ffi::c_void,
    ) -> Hresult {
        if riid.is_null() || ppv.is_null() {
            return E_NOINTERFACE;
        }
        let iid = &*riid;
        let is_unk = iid.data1 == 0x00000000
            && iid.data2 == 0
            && iid.data3 == 0
            && iid.data4 == [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46];
        let is_drop = iid.data1 == IID_IDROPTARGET.data1
            && iid.data2 == IID_IDROPTARGET.data2
            && iid.data3 == IID_IDROPTARGET.data3
            && iid.data4 == IID_IDROPTARGET.data4;
        if is_unk || is_drop {
            *ppv = this as *mut _;
            add_ref(this);
            S_OK
        } else {
            *ppv = ptr::null_mut();
            E_NOINTERFACE
        }
    }

    unsafe extern "system" fn add_ref(this: *mut IDropTarget) -> u32 {
        (*this).ref_count += 1;
        (*this).ref_count
    }

    unsafe extern "system" fn release(this: *mut IDropTarget) -> u32 {
        (*this).ref_count -= 1;
        let c = (*this).ref_count;
        if c == 0 {
            let _ = Box::from_raw(this);
        }
        c
    }

    unsafe extern "system" fn drag_enter(
        this: *mut IDropTarget,
        p_data_obj: *mut std::ffi::c_void,
        _grf_key_state: u32,
        pt: PointL,
        pdw_effect: *mut u32,
    ) -> Hresult {
        let paths = data_object_get_hdrop(p_data_obj).unwrap_or_default();
        (*this).paths = paths
            .into_iter()
            .filter(|p| is_supported_audio(p))
            .collect();
        let (x, y) = client_from_screen((*this).hwnd, pt);
        let zone = (*this).layout.hit_test_physical(x, y);
        let effect = if zone != DropZone::None && !(*this).paths.is_empty() {
            DROPEFFECT_COPY
        } else {
            DROPEFFECT_NONE
        };
        if !pdw_effect.is_null() {
            *pdw_effect = effect;
        }
        ((*this).callback)(OleDropEvent::Hover {
            zone,
            paths: (*this).paths.clone(),
        });
        S_OK
    }

    unsafe extern "system" fn drag_over(
        this: *mut IDropTarget,
        _grf_key_state: u32,
        pt: PointL,
        pdw_effect: *mut u32,
    ) -> Hresult {
        let (x, y) = client_from_screen((*this).hwnd, pt);
        let zone = (*this).layout.hit_test_physical(x, y);
        let effect = if zone != DropZone::None && !(*this).paths.is_empty() {
            DROPEFFECT_COPY
        } else {
            DROPEFFECT_NONE
        };
        if !pdw_effect.is_null() {
            *pdw_effect = effect;
        }
        ((*this).callback)(OleDropEvent::Hover {
            zone,
            paths: (*this).paths.clone(),
        });
        S_OK
    }

    unsafe extern "system" fn drag_leave(this: *mut IDropTarget) -> Hresult {
        (*this).paths.clear();
        ((*this).callback)(OleDropEvent::Leave);
        S_OK
    }

    unsafe extern "system" fn drop(
        this: *mut IDropTarget,
        p_data_obj: *mut std::ffi::c_void,
        _grf_key_state: u32,
        pt: PointL,
        pdw_effect: *mut u32,
    ) -> Hresult {
        let mut paths = data_object_get_hdrop(p_data_obj).unwrap_or_default();
        if paths.is_empty() {
            paths = (*this).paths.clone();
        }
        paths.retain(|p| is_supported_audio(p) || p.extension().is_some());
        // Keep unsupported too so decide_drop can reject with a message;
        // if empty after OS filter, use raw list.
        let (x, y) = client_from_screen((*this).hwnd, pt);
        let zone = (*this).layout.hit_test_physical(x, y);
        if !pdw_effect.is_null() {
            *pdw_effect = if zone != DropZone::None {
                DROPEFFECT_COPY
            } else {
                DROPEFFECT_NONE
            };
        }
        let all = if paths.is_empty() {
            (*this).paths.clone()
        } else {
            paths
        };
        ((*this).callback)(OleDropEvent::Drop {
            zone,
            paths: all,
            client_x: x,
            client_y: y,
        });
        (*this).paths.clear();
        S_OK
    }

    static REGISTERED_HWND: AtomicIsize = AtomicIsize::new(0);

    pub fn try_register(hwnd: isize, layout: SharedDropLayout, callback: OleDropCallback) -> bool {
        if hwnd == 0 {
            return false;
        }
        unsafe {
            let _ = OleInitialize(ptr::null_mut());
            // Revoke any previous registration (winit may own one — try anyway).
            let _ = RevokeDragDrop(hwnd);
            let target = Box::new(IDropTarget {
                vtbl: &VTBL,
                ref_count: 1,
                hwnd,
                layout,
                callback,
                paths: Vec::new(),
            });
            let ptr = Box::into_raw(target);
            let hr = RegisterDragDrop(hwnd, ptr);
            if hr != S_OK {
                // Restore ownership so we don't leak on failure.
                let _ = Box::from_raw(ptr);
                eprintln!("RegisterDragDrop failed: HRESULT 0x{hr:08X}");
                return false;
            }
            REGISTERED_HWND.store(hwnd, Ordering::SeqCst);
            true
        }
    }

    pub fn revoke_if_registered() {
        let hwnd = REGISTERED_HWND.swap(0, Ordering::SeqCst);
        if hwnd != 0 {
            unsafe {
                let _ = RevokeDragDrop(hwnd);
            }
        }
    }

    #[allow(dead_code)]
    const _: () = {
        let _ = (DRAGDROP_S_DROP, DRAGDROP_S_CANCEL, IID_IDATAOBJECT, E_NOTIMPL);
    };
}

#[cfg(windows)]
pub use ole::{revoke_if_registered, try_register};

#[cfg(not(windows))]
pub fn try_register(
    _hwnd: isize,
    _layout: SharedDropLayout,
    _callback: OleDropCallback,
) -> bool {
    false
}

#[cfg(not(windows))]
pub fn revoke_if_registered() {}
