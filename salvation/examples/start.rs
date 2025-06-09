#![allow(dead_code)]

use {
    anyhow::Result,
    salvation::{
        impl_widget_common,
        system::add_interval,
        widgets::{
            button::Button, column::Column, label::Label, scroll_area::ScrollArea,
            text_input::TextInput, window::WindowWidget, Widget, WidgetCommonTyped, WidgetExt,
            WidgetId,
        },
        App,
    },
    std::time::Duration,
};

struct AnotherWidget {
    common: WidgetCommonTyped<Self>,
    counter: i32,
}

// #[derive(Debug)]
// enum Abc {
//     Def,
// }

// impl FormatKey for Abc {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         writeln!(f, "{:?}", self)
//     }
// }

impl Widget for AnotherWidget {
    impl_widget_common!();

    fn new(common: WidgetCommonTyped<Self>) -> Self {
        let mut this = Self { counter: 0, common };
        let callback = this.callback(|this, _event| {
            this.counter += 1;
            println!("counter: {}", this.counter);
            let window = this
                .common
                .add_child_with_key::<WindowWidget>(("window", this.counter))
                .set_title("example");
            println!("window {:?}", window.id());
            let label = window
                .common_mut()
                .add_child::<Label>()
                .set_column(0)
                .set_row(0)
                .set_text(format!("counter: {}", this.counter));
            println!("label {:?}", label.id());
            Ok(())
        });
        let button = this
            .common_mut()
            .add_child_with_key::<Button>("button")
            .set_column(0)
            .set_row(1);
        button.set_text("another button");
        button.on_triggered(callback);
        this
    }
}

struct RootWidget {
    common: WidgetCommonTyped<Self>,
    button_id: WidgetId<Button>,
    column2_id: WidgetId<Column>,
    button21_id: WidgetId<Button>,
    button22_id: WidgetId<Button>,
    flag_column: bool,
    flag_button21: bool,
    i: i32,
    label2_id: WidgetId<Label>,
}

impl RootWidget {
    fn inc(&mut self) -> Result<()> {
        self.i += 1;
        if let Ok(widget) = self.common.widget(self.button21_id) {
            widget.set_text(format!("i = {}", self.i));
        }
        Ok(())
    }

    fn button_clicked(&mut self, data: (), k: u32) -> Result<()> {
        println!("callback! {:?}, {}", data, k);
        let button = self.common.widget(self.button_id)?;
        button.set_text(format!("ok {}", if k == 1 { "1" } else { "22222" }));

        if k == 1 {
            self.flag_column = !self.flag_column;
            self.common
                .widget(self.column2_id)?
                .set_enabled(self.flag_column);
            println!("set enabled {:?} {:?}", self.column2_id, self.flag_column);
        } else {
            self.flag_button21 = !self.flag_button21;
            self.common
                .widget(self.button21_id)?
                .set_enabled(self.flag_button21);
            println!(
                "set enabled {:?} {:?}",
                self.button21_id, self.flag_button21
            );
        }
        Ok(())
    }
}

impl Widget for RootWidget {
    impl_widget_common!();

    fn new(mut common: WidgetCommonTyped<Self>) -> Self {
        let id = common.id();

        let window = common.add_child::<WindowWidget>().set_title("example");

        let root = window
            .common_mut()
            .add_child::<Column>()
            .set_column(0)
            .set_row(0);

        root.common_mut()
            .add_child::<TextInput>()
            .set_column(0)
            .set_row(0)
            .set_text("Hello, Rust! 🦀😂\n");
        root.common_mut()
            .add_child::<TextInput>()
            .set_column(0)
            .set_row(1)
            .set_text("Hebrew name Sarah: שרה.");

        /*
        let btn = Button::new("btn1")
            .with_icon(icon)
            .with_alignment(Al::Right)
            .with_on_clicked(slot)
            .split_id()
            .boxed();
        root.add(btn.widget);

        Self {
            btn_id: btn.id,
        }


        */

        let button_id = root
            .common_mut()
            .add_child::<Button>()
            .set_column(0)
            .set_row(2)
            .set_text("btn1")
            .set_auto_repeat(true)
            .on_triggered(id.callback(|this, event| this.button_clicked(event, 1)))
            .id();

        root.common_mut()
            .add_child::<Button>()
            .set_column(0)
            .set_row(3)
            .set_text("btn2")
            .on_triggered(id.callback(|this, event| this.button_clicked(event, 2)));

        let column2 = root
            .common_mut()
            .add_child::<Column>()
            .set_column(0)
            .set_row(4);
        let button21_id = column2
            .common_mut()
            .add_child::<Button>()
            .set_column(0)
            .set_row(0)
            .set_text("btn21")
            .on_triggered(id.callback(|_, _| {
                println!("click!");
                Ok(())
            }))
            .id();

        let button22_id = column2
            .common_mut()
            .add_child::<Button>()
            .set_column(0)
            .set_row(1)
            .set_text("btn22")
            .id();
        let column2_id = column2.id();

        root.common_mut()
            .add_child::<AnotherWidget>()
            .set_column(0)
            .set_row(5);

        let label2_id = root
            .common_mut()
            .add_child::<Label>()
            .set_column(0)
            .set_row(6)
            .set_text("ok")
            .id();

        let scroll_area = root
            .common_mut()
            .add_child::<ScrollArea>()
            .set_column(0)
            .set_row(7);
        let content = scroll_area.set_content::<Column>();
        for i in 1..=80 {
            content
                .common_mut()
                .add_child::<Button>()
                .set_column(0)
                .set_row(i)
                .set_text(format!("btn btn btn btn btn btn btn btn btn btn{i}"));
        }

        add_interval(Duration::from_secs(2), id.callback(|this, _| this.inc()));

        RootWidget {
            common,
            button_id,
            column2_id,
            button21_id,
            button22_id,
            flag_column: true,
            flag_button21: true,
            i: 0,
            label2_id,
        }
    }
}

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info")
    }
    env_logger::init();
    App::new()
        .run(|r| {
            r.common_mut().add_child::<RootWidget>();
            Ok(())
        })
        .unwrap();
}
