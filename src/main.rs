use std::ffi::{CString, c_char, c_int};

use glib_sys::gpointer;
use gobject_sys::{GCallback, g_signal_connect_data};
use wnck::{Screen, Window};

#[cfg(feature = "notify")]
mod notify;

mod wmctrl;
mod wnck;

extern "C" fn workspace_callback(
    _screen: *mut wnck_sys::WnckScreen,
    _data: *mut gobject_sys::GObject,
    user_data: glib_sys::gpointer,
) {
    let workspaces = unsafe { &mut *user_data.cast::<DynamicWorkspaces>() };
    workspaces.handle_dynamic_workspaces();
}

#[cfg(feature = "notify")]
extern "C" fn notify_callback(
    _screen: *mut wnck_sys::WnckScreen,
    _data: *mut gobject_sys::GObject,
    user_data: glib_sys::gpointer,
) {
    let workspaces = unsafe { &mut *user_data.cast::<DynamicWorkspaces>() };
    workspaces.update_notification();
}

struct DynamicWorkspaces {
    debug: bool,
    notify: bool,
    window_blacklist: &'static [&'static str],
    window_classrole_blacklist: &'static [(&'static str, &'static str)],
    last: usize,
    screen: Screen,
}

impl DynamicWorkspaces {
    pub fn new(debug: bool, notify: bool) -> Self {
        let screen = Screen::get_default();
        screen.force_update();

        unsafe {
            while gtk_sys::gtk_events_pending() != 0 {
                gtk_sys::gtk_main_iteration_do(0);
            }
        }

        #[cfg(feature = "notify")]
        if notify {
            notify::default_notification();
        }

        Self {
            debug,
            notify,
            window_blacklist: &[
                "Skriveboard",
                "Desktop",
                "xfdashboard",
                "xfce4-panel",
                "plank",
                "xfce4-notifyd",
                "Whisker Menu",
            ],
            window_classrole_blacklist: &[("tilix", "quake")],
            last: 0,
            screen,
        }
    }

    #[cfg(feature = "notify")]
    pub fn update_notification(&self) {
        if let Some(workspace_num) = self.screen.get_active_workspace() {
            let workspace_num = workspace_num.get_number() + 1;
            notify::update_notification(workspace_num);
        }
    }

    /// Main logic for handling of dynamic workspaces
    pub fn handle_dynamic_workspaces(&mut self) {
        // Gets the current workspaces
        let workspaces = self.screen.get_workspaces();
        let workspaces_len = workspaces.len();

        // Initiates necessary scope variables and counts the windows on the relevant workspaces
        if !workspaces.is_empty() {
            let mut last = 0;
            let mut next_last = 0;
            // Removes blacklisted windows from the list of visible windows
            let windows = self.remove_blacklist(&self.screen.get_windows());

            // Counts windows
            for window in windows {
                // Checks if the window is on the last workspace
                if window.is_on_workspace(&workspaces[workspaces.len() - 1]) {
                    last += 1;
                }
                if workspaces_len > 1 {
                    // Checks if the window is on the workspace before the last
                    if window.is_on_workspace(&workspaces[workspaces.len() - 2]) {
                        next_last += 1;
                    }
                }
            }

            // Main logical operations for removing last/last two workspaces
            if last > 0 {
                self.add_workspace(workspaces_len);
            }
            if workspaces_len > 1 && last == 0 && next_last == 0 {
                self.pop_workspace(workspaces_len);
            }
        }

        // Refresh the current workspaces and windows
        let workspaces = self.screen.get_workspaces();
        let workspaces_len = workspaces.len();

        // If there are more than 2 workspaces, iterate through all the workspaces except the last
        // one and check if they are empty. if they are, remove them.
        if workspaces_len > 2 {
            let windows = self.remove_blacklist(&self.screen.get_windows());
            for (idx, workspace) in workspaces
                .iter()
                .take(workspaces.len().saturating_sub(1))
                .enumerate()
            {
                if self.screen.get_active_workspace().as_ref() != Some(workspace)
                    && self.screen.get_workspaces().last() != Some(workspace)
                {
                    let mut workspace_empty = true;
                    for window in &windows {
                        if window.is_on_workspace(workspace) {
                            workspace_empty = false;
                            break;
                        }
                    }
                    if workspace_empty {
                        let workspaces = self.screen.get_workspaces();
                        if let Some(last_workspace) = workspaces.last() {
                            if workspace != last_workspace {
                                self.remove_workspace_by_index(idx);
                            }
                        }
                    }
                }
            }
        }

        // Update last workspace.
        if let Some(workspace) = self.screen.get_active_workspace() {
            self.last = workspace.get_number() as usize;
        }
    }

    /// Removes blacklisted windows from the list of visible windows
    pub fn remove_blacklist(&self, windows: &[Window]) -> Vec<Window> {
        let keep: Vec<Window> = windows
            .iter()
            .filter(|window| {
                if window.is_sticky() {
                    return false;
                }
                if self.window_blacklist.contains(&window.get_name().as_ref()) {
                    return false;
                }
                if !window.get_role().is_empty()
                    && self.window_classrole_blacklist.contains(&(
                        window.get_class_instance_name().as_ref(),
                        window.get_role().as_ref(),
                    ))
                {
                    return false;
                }
                true
            })
            .cloned()
            .collect();

        if self.debug {
            for window in &keep {
                println!("{}", window.get_name());
            }
        }

        keep
    }

    /// Functions for handling adding/removal of workspaces. These functions just work as an
    /// interface to send shell commands with wmctrl.
    pub fn add_workspace(&self, workspaces_len: usize) {
        let _ = wmctrl::set_desktop_count(workspaces_len + 1);
    }

    pub fn pop_workspace(&self, workspaces_len: usize) {
        if self.screen.get_workspaces().len() > 2 {
            let _ = wmctrl::set_desktop_count(workspaces_len - 1);
        }
    }

    /// Removes a workspace by index using wmctrl
    pub fn remove_workspace_by_index(&self, index: usize) {
        // Get current workspace number
        let workspace_num = self.screen.get_active_workspace().map(|ws| ws.get_number());
        // Get current workspaces using wmctrl
        // The user probably has at most like 10 desktops.
        #[allow(clippy::naive_bytecount)]
        let workspaces = wmctrl::list_desktops()
            .stdout
            .iter()
            .filter(|&&b| b == b'\n')
            .count();
        // Get current windows and their workspaces.
        // Filter out the windows that don't have workspaces or are on any workspace on a lower
        // index than the workspace to be removed
        let windows: Vec<Window> = self
            .screen
            .get_windows()
            .into_iter()
            .filter(|window| {
                if let Some(ws) = window.get_workspace() {
                    ws.get_number() > index as i32
                } else {
                    false
                }
            })
            .collect();

        for window in windows {
            if let Some(workspace) = window.get_workspace() {
                // Move the windows that are left one workspace to the left
                window.move_to_workspace(
                    &self.screen.get_workspaces()[workspace.get_number() as usize - 1],
                );
            }
        }
        self.pop_workspace(workspaces);

        // Make sure you stay on the workspace
        if let Some(workspace_num) = workspace_num {
            if self.last < workspace_num as usize {
                let _ = wmctrl::switch_desktop(index.to_string());
            }
        }
    }

    pub fn connect_signals(&mut self) {
        let _ = wmctrl::set_desktop_count(1);
        let screen_ptr = self.screen.as_mut_ptr();

        fn connect_signal(
            screen: *mut gobject_sys::GObject,
            signal: &str,
            handler: GCallback,
            data: gpointer,
        ) {
            let c_signal = CString::new(signal).unwrap();
            unsafe {
                g_signal_connect_data(screen, c_signal.as_ptr(), handler, data, None, 0);
            }
        }

        let self_ptr = self as *mut _ as gpointer;
        let gobject = screen_ptr.cast::<gobject_sys::GObject>();

        let callback: GCallback = unsafe {
            Some(std::mem::transmute::<*const (), unsafe extern "C" fn()>(
                workspace_callback as *const (),
            ))
        };

        let signals = [
            "active-workspace-changed",
            "workspace-created",
            "workspace-destroyed",
            "window-opened",
            "window-closed",
        ];

        #[cfg(feature = "notify")]
        if self.notify {
            let notify_callback: GCallback = unsafe {
                Some(std::mem::transmute::<*const (), unsafe extern "C" fn()>(
                    notify_callback as *const (),
                ))
            };
            connect_signal(
                gobject,
                "active-workspace-changed",
                notify_callback,
                self_ptr,
            );
        }

        for signal in &signals {
            connect_signal(gobject, signal, callback, self_ptr);
        }
    }
}

fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    let mut debug = false;
    let mut notify = true;

    let cstrings = args
        .iter()
        .map(|arg| {
            match arg.as_str() {
                "--debug" => {
                    println!("Debug mode enabled");
                    debug = true;
                }
                "--no-notify" => {
                    println!("Notifications disabled");
                    notify = false;
                }
                _ => {}
            }
            CString::new(arg.as_str()).unwrap()
        })
        .collect::<Vec<CString>>();

    let mut c_args: Vec<*mut c_char> = cstrings
        .iter()
        .map(|cstr| cstr.as_ptr().cast_mut())
        .collect();

    let mut argc = c_args.len() as c_int;
    let mut argv_ptr = c_args.as_mut_ptr();

    if unsafe { gdk_sys::gdk_init_check(&raw mut argc, &raw mut argv_ptr) } == 0 {
        eprintln!("`gdk_init_check` failed to start");
        std::process::exit(1);
    }

    gtk::init().expect("Failed to initialize GTK");

    println!("Started workspace indicator");
    let mut workspaces = DynamicWorkspaces::new(debug, notify);
    workspaces.connect_signals();

    gtk::main();
}
