use {
    salvation::{
        impl_widget_common,
        widgets::{
            button::Button, menu::Menu, window::WindowWidget, Widget,
            WidgetCommonTyped, WidgetExt,
        },
    },
    salvation_test_kit::context::Context,
};

pub struct RootWidget {
    common: WidgetCommonTyped<Self>,
}

impl RootWidget {
    fn on_triggered(&mut self, _event: ()) -> anyhow::Result<()> {
        self.common.add_child::<Menu>();
        Ok(())
    }
}

impl Widget for RootWidget {
    impl_widget_common!();

    fn new(mut common: WidgetCommonTyped<Self>) -> Self {
        let id = common.id();
        let window = common.add_child::<WindowWidget>().set_title(module_path!());

        window
            .common_mut()
            .add_child::<Button>()
            .set_column(0)
            .set_row(0)
            .set_text("test")
            .on_triggered(id.callback(Self::on_triggered));

        Self { common }
    }
}

#[salvation_test_kit::test]
fn menu(ctx: &mut Context) -> anyhow::Result<()> {
    ctx.run(|r| {
        r.common_mut().add_child::<RootWidget>();
        Ok(())
    })?;
    let window = ctx.wait_for_window_by_pid()?;
    window.close()?;
    Ok(())
}
