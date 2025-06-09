use {
    salvation::{
        impl_widget_common,
        widgets::{
            label::Label, window::WindowWidget, Widget, WidgetCommonTyped, WidgetExt,
        },
    },
    salvation_test_kit::context::Context,
};

pub struct RootWidget {
    common: WidgetCommonTyped<Self>,
}

impl Widget for RootWidget {
    impl_widget_common!();

    fn new(mut common: WidgetCommonTyped<Self>) -> Self {
        let window = common.add_child::<WindowWidget>().set_title(module_path!());

        window
            .common_mut()
            .add_child::<Label>()
            .set_column(0)
            .set_row(0)
            .set_text("Test");

        Self { common }
    }
}

#[salvation_test_kit::test]
pub fn label(ctx: &mut Context) -> anyhow::Result<()> {
    ctx.run(|r| {
        r.common_mut().add_child::<RootWidget>();
        Ok(())
    })?;
    let mut window = ctx.wait_for_window_by_pid()?;
    ctx.snapshot(&mut window, "label")?;
    window.close()?;
    Ok(())
}
