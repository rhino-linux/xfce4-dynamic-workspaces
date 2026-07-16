use std::borrow::Cow;
use std::ffi::CStr;

use wnck_sys::{
    WnckScreen, WnckWindow, WnckWorkspace, wnck_screen_force_update,
    wnck_screen_get_active_workspace, wnck_screen_get_default, wnck_screen_get_windows,
    wnck_screen_get_workspace_count, wnck_screen_get_workspaces,
    wnck_window_get_class_instance_name, wnck_window_get_name, wnck_window_get_role,
    wnck_window_get_workspace, wnck_window_is_on_workspace, wnck_window_is_sticky,
    wnck_window_move_to_workspace, wnck_workspace_activate, wnck_workspace_change_name,
    wnck_workspace_get_height, wnck_workspace_get_name, wnck_workspace_get_number,
    wnck_workspace_get_screen, wnck_workspace_get_width, wnck_workspace_is_virtual,
};

pub struct Screen {
    screen: *mut WnckScreen,
}

impl Screen {
    /// Get default screen handle.
    pub fn get_default() -> Self {
        Self {
            screen: unsafe { wnck_screen_get_default() },
        }
    }

    /// Get mutable inner pointer.
    pub const fn as_mut_ptr(&mut self) -> *mut WnckScreen {
        self.screen
    }

    /// Force update the screen.
    pub fn force_update(&self) {
        unsafe {
            wnck_screen_force_update(self.screen);
        }
    }

    /// Get active workspace that screen is on.
    ///
    /// # Returns
    /// The optional workspace that the screen is on.
    pub fn get_active_workspace(&self) -> Option<Workspace> {
        let ptr = unsafe { wnck_screen_get_active_workspace(self.screen) };
        if ptr.is_null() {
            None
        } else {
            Some(Workspace { workspace: ptr })
        }
    }

    /// Get a list of workspaces on a screen.
    ///
    /// # Note
    /// There is no guarantee that the workspaces will be valid by the time the caller uses them,
    /// so be warned.
    pub fn get_workspaces(&self) -> Vec<Workspace> {
        let work_count = unsafe { wnck_screen_get_workspace_count(self.screen) };
        let mut out = Vec::with_capacity(work_count as usize);

        unsafe {
            let mut list = wnck_screen_get_workspaces(self.screen);
            while !list.is_null() {
                let node = &*list;
                let workspace_ptr = node.data.cast::<WnckWorkspace>();
                out.push(Workspace {
                    workspace: workspace_ptr,
                });
                list = node.next;
            }
        }

        out
    }

    pub fn get_windows(&self) -> Vec<Window> {
        let mut out = vec![];
        unsafe {
            let mut list = wnck_screen_get_windows(self.screen);
            while !list.is_null() {
                let node = &*list;
                let workspace_str = node.data.cast::<WnckWindow>();
                out.push(Window {
                    window: workspace_str,
                });
                list = node.next;
            }
        }

        out
    }
}

#[derive(PartialEq)]
pub struct Workspace {
    workspace: *mut WnckWorkspace,
}

impl Workspace {
    pub fn get_number(&self) -> i32 {
        unsafe { wnck_workspace_get_number(self.workspace) }
    }

    pub fn get_size(&self) -> (i32, i32) {
        unsafe {
            (
                wnck_workspace_get_width(self.workspace),
                wnck_workspace_get_height(self.workspace),
            )
        }
    }

    pub fn get_screen(&self) -> Screen {
        unsafe {
            Screen {
                screen: wnck_workspace_get_screen(self.workspace),
            }
        }
    }

    pub fn get_name(&self) -> Cow<'_, str> {
        let c_str = unsafe { wnck_workspace_get_name(self.workspace) };
        unsafe { CStr::from_ptr(c_str).to_string_lossy() }
    }

    pub fn change_name<S: AsRef<CStr>>(&mut self, name: S) {
        unsafe {
            wnck_workspace_change_name(self.workspace, name.as_ref().as_ptr());
        }
    }

    /// Attempt to make this the active workspace.
    pub fn activate(&self, timestamp: u32) {
        unsafe {
            wnck_workspace_activate(self.workspace, timestamp);
        }
    }

    pub fn is_virtual(&self) -> bool {
        unsafe { wnck_workspace_is_virtual(self.workspace) != 0 }
    }
}

#[derive(PartialEq, Eq)]
pub struct Window {
    window: *mut WnckWindow,
}

impl Clone for Window {
    fn clone(&self) -> Self {
        Self {
            window: self.window,
        }
    }
}

impl Window {
    pub fn is_on_workspace(&self, workspace: &Workspace) -> bool {
        unsafe { wnck_window_is_on_workspace(self.window, workspace.workspace) != 0 }
    }

    pub fn is_sticky(&self) -> bool {
        unsafe { wnck_window_is_sticky(self.window) != 0 }
    }

    pub fn get_name(&self) -> Cow<'_, str> {
        let c_str = unsafe { wnck_window_get_name(self.window) };
        unsafe { CStr::from_ptr(c_str).to_string_lossy() }
    }

    pub fn get_class_instance_name(&self) -> Cow<'_, str> {
        let c_str = unsafe { wnck_window_get_class_instance_name(self.window) };
        unsafe { CStr::from_ptr(c_str).to_string_lossy() }
    }

    pub fn get_role(&self) -> Cow<'_, str> {
        let c_str = unsafe { wnck_window_get_role(self.window) };
        if c_str.is_null() {
            Cow::Borrowed("")
        } else {
            unsafe { CStr::from_ptr(c_str).to_string_lossy() }
        }
    }

    pub fn move_to_workspace(&self, workspace: &Workspace) {
        unsafe {
            wnck_window_move_to_workspace(self.window, workspace.workspace);
        }
    }

    pub fn get_workspace(&self) -> Option<Workspace> {
        let ptr = unsafe { wnck_window_get_workspace(self.window) };
        if ptr.is_null() {
            None
        } else {
            Some(Workspace { workspace: ptr })
        }
    }
}
