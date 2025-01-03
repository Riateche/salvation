use {
    crate::context::Context,
    salvation::{
        impl_widget_common,
        widgets::{
            padding_box::PaddingBox, text_input::TextInput, Widget, WidgetCommon, WidgetExt,
        },
        WindowAttributes,
    },
};

pub struct RootWidget {
    common: WidgetCommon,
}

impl RootWidget {
    pub fn new() -> Self {
        let mut common = WidgetCommon::new::<Self>();
        let input = TextInput::new("Hello world");
        common.add_child(
            PaddingBox::new(input.boxed())
                .with_window(WindowAttributes::default().with_title(module_path!()))
                .boxed(),
            Default::default(),
        );
        Self {
            common: common.into(),
        }
    }
}

impl Widget for RootWidget {
    impl_widget_common!();
}

pub fn check(ctx: &mut Context) -> anyhow::Result<()> {
    ctx.set_blinking_expected(true);
    let mut window = ctx.wait_for_window_by_pid()?;
    // Workaround for winit issue:
    // https://github.com/rust-windowing/winit/issues/2841
    window.minimize()?;
    window.activate()?;
    ctx.snapshot(&mut window, "window with text input - text Hello world")?;
    ctx.connection.key("Right")?;
    ctx.snapshot(&mut window, "cursor moved to the right of H")?;
    ctx.connection.key("Shift+Right")?;
    ctx.set_blinking_expected(false);
    ctx.snapshot(&mut window, "selected e")?;
    ctx.connection.key("Right")?;
    ctx.set_blinking_expected(true);
    ctx.snapshot(
        &mut window,
        "cleared selection and cursor moved to the right of He",
    )?;
    ctx.connection.key("Left")?;
    ctx.snapshot(&mut window, "cursor moved to the right of H")?;
    ctx.connection.key("Ctrl+Right")?;
    ctx.snapshot(&mut window, "cursor moved to the right of Hello")?;
    ctx.connection.key("Ctrl+Right")?;
    ctx.snapshot(&mut window, "cursor moved to the end")?;
    ctx.connection.key("Ctrl+Left")?;
    ctx.snapshot(
        &mut window,
        "cursor moved to the right of Hello after space",
    )?;
    ctx.connection.key("Ctrl+Left")?;
    ctx.snapshot(&mut window, "cursor moved to the start")?;
    ctx.connection.key("End")?;
    ctx.snapshot(&mut window, "cursor moved to the end")?;
    ctx.connection.key("Shift+Left")?;
    ctx.set_blinking_expected(false);
    ctx.snapshot(&mut window, "selected d")?;
    ctx.connection.key("Left")?;
    ctx.set_blinking_expected(true);
    ctx.snapshot(
        &mut window,
        "cleared selection and cursor moved to the right of worl",
    )?;
    ctx.connection.key("Ctrl+Shift+Left")?;
    ctx.set_blinking_expected(false);
    ctx.snapshot(&mut window, "selected worl")?;
    ctx.connection.key("End")?;
    ctx.connection.type_text(" Lorem Ipsum")?;
    ctx.set_blinking_expected(true);
    ctx.snapshot(&mut window, "added space Lorem Ipsum to the end")?;
    // Checking horizontal scroll.
    ctx.connection.key("Ctrl+Left")?;
    ctx.connection.key("Ctrl+Left")?;
    ctx.connection.key("Ctrl+Left")?;
    ctx.snapshot(
        &mut window,
        "cursor moved to the right of Hello after space",
    )?;
    ctx.connection.key("Left")?;
    ctx.snapshot(
        &mut window,
        "cursor moved to the right of Hello and scrolled",
    )?;
    ctx.connection.key("Left")?;
    ctx.snapshot(
        &mut window,
        "cursor moved to the right of Hell and scrolled",
    )?;
    ctx.connection.key("Left")?;
    ctx.snapshot(&mut window, "cursor moved to the right of Hel and scrolled")?;

    window.close()?;
    Ok(())
}
