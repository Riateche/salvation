pub use super::scroll_bar::RootWidget;

use crate::context::Context;

pub fn check(ctx: &mut Context) -> anyhow::Result<()> {
    let mut window = ctx.wait_for_window_by_pid()?;
    // Workaround for winit issue:
    // https://github.com/rust-windowing/winit/issues/2841
    window.minimize()?;
    window.activate()?;
    window.resize(160, 66)?;
    ctx.snapshot(&mut window, "scroll bar")?;

    window.mouse_move(140, 20)?;
    ctx.snapshot(&mut window, "highlighted right arrow")?;
    ctx.connection.mouse_down(1)?;
    ctx.snapshot(&mut window, "pressed right arrow - step right by 5")?;
    ctx.connection.mouse_up(1)?;
    ctx.snapshot(&mut window, "released right arrow - no auto repeat")?;

    ctx.connection.mouse_down(1)?;
    ctx.snapshot(&mut window, "pressed right arrow - step right by 5")?;
    ctx.snapshot(&mut window, "first auto repeat")?;
    ctx.snapshot(&mut window, "second auto repeat")?;
    ctx.snapshot(&mut window, "third auto repeat")?;
    ctx.connection.mouse_up(1)?;
    ctx.snapshot(&mut window, "released right arrow - no more auto repeats")?;

    window.close()?;
    Ok(())
}
