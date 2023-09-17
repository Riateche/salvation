#![allow(dead_code)]

use salvation::{
    event_loop::{self, CallbackContext},
    widgets::{
        button::Button, column::Column, label::Label, text_input::TextInput, Widget, WidgetExt,
        WidgetId,
    },
    window::create_window,
};
use winit::window::WindowBuilder;

struct AnotherState {
    counter: i32,
}

impl AnotherState {
    fn new(ctx: &mut CallbackContext<Self>) -> (Self, Box<dyn Widget>) {
        let another_state = AnotherState { counter: 0 };
        let mut btn = Button::new("another button");
        btn.on_clicked(ctx.callback(|state, _ctx, _event| {
            state.counter += 1;
            println!("counter: {}", state.counter);
            create_window(
                WindowBuilder::new().with_title("example"),
                Some(Box::new(Label::new(format!("counter: {}", state.counter)))),
            );
        }));
        (another_state, Box::new(btn))
    }
}

struct State {
    another_state: AnotherState,
    button_id: WidgetId<Button>,
}

impl State {
    fn new(ctx: &mut CallbackContext<Self>) -> Self {
        let mut root = Column::new();
        // let w1 =
        //     Image::load_png("1.png").unwrap();
        // root.add(
        //     Rect {
        //         top_left: Point { x: 20, y: 30 },
        //         size: Size { x: 300, y: 300 },
        //     },
        //     Box::new(w1),
        // );

        // let w2 = TextInput::new("Hello, Rust! 🦀 one two three four five\n");
        let w2 = TextInput::new("Hello, Rust! 🦀\n");
        root.add(Box::new(w2));
        let w3 = TextInput::new("Hebrew \nname Sarah: שרה, spelled");
        root.add(Box::new(w3));

        let mut btn1 = Button::new("btn1");
        let button_id = btn1.id();
        btn1.on_clicked(ctx.callback(|state, ctx, event| {
            state.button_clicked2(ctx, event, 1);
        }));
        root.add(Box::new(btn1));

        let mut btn2 = Button::new("btn2");
        // btn2.on_clicked(ctx.callback_maker.add(Self::button_clicked));
        btn2.on_clicked(ctx.callback(|state, ctx, event| {
            state.button_clicked2(ctx, event, 2);
        }));
        root.add(Box::new(btn2));

        let (another_state, btn3) =
            AnotherState::new(&mut ctx.map_state(|state| Some(&mut state.another_state)));
        root.add(btn3);

        create_window(
            WindowBuilder::new().with_title("example"),
            Some(Box::new(root)),
        );
        State {
            another_state,
            button_id,
        }
    }

    // fn button_clicked(&mut self, _ctx: &mut CallbackContext<Self>, data: String) {
    //     println!("callback! {:?}", data);
    // }

    fn button_clicked2(&mut self, ctx: &mut CallbackContext<Self>, data: String, k: u32) {
        println!("callback! {:?}, {}", data, k);
        let button = ctx.get_widget_by_id_mut(self.button_id).unwrap();
        button.set_text(&format!("ok {k}"));
    }
}

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info")
    }
    env_logger::init();
    event_loop::run(State::new);
}
