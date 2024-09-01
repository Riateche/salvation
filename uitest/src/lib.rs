use anyhow::bail;
use std::{
    process::Command,
    thread::sleep,
    time::{Duration, Instant},
};
use xcap::image::RgbaImage;

use x11rb::{
    protocol::xproto::{Atom, ConnectionExt},
    rust_connection::RustConnection,
};

const SINGLE_WAIT_DURATION: Duration = Duration::from_millis(200);
const DEFAULT_WAIT_DURATION: Duration = Duration::from_secs(5);

pub struct Connection {
    connection: RustConnection,
    net_wm_pid: Atom,
    cardinal: Atom,
    wait_duration: Duration,
}

fn get_or_intern_atom(conn: &RustConnection, name: &[u8]) -> Atom {
    let result = conn
        .intern_atom(false, name)
        .expect("Failed to intern atom")
        .reply()
        .expect("Failed receive interned atom");

    result.atom
}

impl Connection {
    #[allow(clippy::new_without_default)]
    pub fn new() -> anyhow::Result<Self> {
        let (connection, _screen_num) = x11rb::connect(None)?;
        let net_wm_pid = get_or_intern_atom(&connection, b"_NET_WM_PID");
        let cardinal = get_or_intern_atom(&connection, b"CARDINAL");
        Ok(Self {
            connection,
            net_wm_pid,
            cardinal,
            wait_duration: DEFAULT_WAIT_DURATION,
        })
    }

    pub fn all_windows(&self) -> anyhow::Result<Vec<Window>> {
        xcap::Window::all()?
            .into_iter()
            .map(|w| Window::new(self, w))
            .collect()
    }

    pub fn windows_by_pid(&self, pid: u32) -> anyhow::Result<Vec<Window>> {
        let windows = self.all_windows()?;
        Ok(windows.into_iter().filter(|w| w.pid == pid).collect())
    }

    pub fn wait_for_windows_by_pid(&self, pid: u32) -> anyhow::Result<Vec<Window>> {
        let started = Instant::now();
        while started.elapsed() < self.wait_duration {
            let windows = self.windows_by_pid(pid)?;
            if !windows.is_empty() {
                return Ok(windows);
            }
            sleep(SINGLE_WAIT_DURATION);
        }
        bail!(
            "couldn't find a window with pid={} after {:?}",
            pid,
            self.wait_duration
        );
    }

    pub fn active_window_id(&self) -> anyhow::Result<u32> {
        let output = Command::new("xdotool").arg("getactivewindow").output()?;
        if !output.status.success() {
            bail!("xdotool failed: {:?}", output);
        }
        Ok(String::from_utf8(output.stdout)?.trim().parse()?)
    }

    pub fn mouse_click(&self, button: u32) -> anyhow::Result<()> {
        let status = Command::new("xdotool")
            .arg("click")
            .arg(button.to_string())
            .status()?;
        if !status.success() {
            bail!("xdotool failed: {:?}", status);
        }
        Ok(())
    }

    pub fn mouse_down(&self, button: u32) -> anyhow::Result<()> {
        let status = Command::new("xdotool")
            .arg("mousedown")
            .arg(button.to_string())
            .status()?;
        if !status.success() {
            bail!("xdotool failed: {:?}", status);
        }
        Ok(())
    }

    pub fn mouse_up(&self, button: u32) -> anyhow::Result<()> {
        let status = Command::new("xdotool")
            .arg("mouseup")
            .arg(button.to_string())
            .status()?;
        if !status.success() {
            bail!("xdotool failed: {:?}", status);
        }
        Ok(())
    }
}

pub struct Window {
    pid: u32,
    inner: xcap::Window,
    //...
}

impl Window {
    fn new(connection: &Connection, inner: xcap::Window) -> anyhow::Result<Self> {
        let pid = connection
            .connection
            .get_property(
                false,
                inner.id(),
                connection.net_wm_pid,
                connection.cardinal,
                0,
                u32::MAX,
            )?
            .reply()?
            .value32()
            .unwrap()
            .next()
            .unwrap();
        Ok(Self { pid, inner })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// The window id
    pub fn id(&self) -> u32 {
        self.inner.id()
    }
    /// The window app name
    pub fn app_name(&self) -> &str {
        self.inner.app_name()
    }
    /// The window title
    pub fn title(&self) -> &str {
        self.inner.title()
    }
    /// The window x coordinate.
    pub fn x(&self) -> i32 {
        self.inner.x()
    }
    /// The window x coordinate.
    pub fn y(&self) -> i32 {
        self.inner.y()
    }
    /// The window pixel width.
    pub fn width(&self) -> u32 {
        self.inner.width()
    }
    /// The window pixel height.
    pub fn height(&self) -> u32 {
        self.inner.height()
    }
    /// The window is minimized.
    pub fn is_minimized(&self) -> bool {
        self.inner.is_minimized()
    }
    /// The window is maximized.
    pub fn is_maximized(&self) -> bool {
        self.inner.is_maximized()
    }

    pub fn capture_image(&self) -> anyhow::Result<RgbaImage> {
        Ok(self.inner.capture_image()?)
    }

    pub fn activate(&self) -> anyhow::Result<()> {
        let status = Command::new("xdotool")
            .arg("windowactivate")
            .arg("--sync")
            .arg(self.id().to_string())
            .status()?;
        if !status.success() {
            bail!("xdotool failed: {:?}", status);
        }

        // let status = Command::new("xdotool")
        //     .arg("windowraise")
        //     .arg(self.id().to_string())
        //     .status()?;
        // if !status.success() {
        //     bail!("xdotool failed: {:?}", status);
        // }
        Ok(())
    }

    pub fn mouse_move(&self, x: u32, y: u32) -> anyhow::Result<()> {
        let status = Command::new("xdotool")
            .arg("mousemove")
            .arg("--window")
            .arg(self.id().to_string())
            .arg("--sync")
            .arg(x.to_string())
            .arg(y.to_string())
            .status()?;
        if !status.success() {
            bail!("xdotool failed: {:?}", status);
        }
        Ok(())
    }

    pub fn close(&self) -> anyhow::Result<()> {
        // `xdotool windowclose` doesn't work properly
        let status = Command::new("wmctrl")
            .arg("-i")
            .arg("-c")
            .arg(self.id().to_string())
            .status()?;
        if !status.success() {
            bail!("wmctrl failed: {:?}", status);
        }
        Ok(())
    }
}