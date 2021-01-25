use druid::{widget::{Button, Flex, Painter, Slider}};
use druid::{AppLauncher, PlatformError, Widget, WidgetExt, WindowDesc, RenderContext};
use basedrop::Collector;
use crate::{sample_player::SamplePlayerController};
use std::sync::{Arc, Mutex};
use druid::kurbo::{BezPath, Affine};
use std::marker;

struct PlayheadController;
impl<W: Widget<UiData>> druid::widget::Controller <UiData, W> for PlayheadController {
    fn event(&mut self, child: &mut W, ctx: &mut druid::EventCtx, event: &druid::Event, data: &mut UiData, env: &druid::Env) {
        if let Ok(controller) = data.controller.try_lock() {
            if let Some(file) = controller.file.as_ref() {
                data.play_pos = (controller.playhead() as f64) / (file.num_samples as f64);
            }
        }
        child.event(ctx, event, data, env)
    }
    fn update(
        &mut self,
        child: &mut W,
        ctx: &mut druid::UpdateCtx<'_, '_>,
        old_data: &UiData,
        data: &UiData,
        env: &druid::Env
    ) {
        if let Ok(mut controller) = data.controller.try_lock() {
            if let Some(file) = controller.file.as_ref() {
                let playhead = (file.num_samples as f64) / (file.sample_rate) * data.play_pos;
                controller.seek(playhead);   
            }
        }
        child.update(ctx, old_data, data, env);
    }
}

#[derive(druid::Data, druid::Lens, Clone)]
pub struct UiData {
    controller: Arc<Mutex<SamplePlayerController>>,
    is_playing: bool,
    play_pos:f64,
}

pub fn run(_gc:Collector, controller:SamplePlayerController) -> Result<(), PlatformError> {
    let main_window = WindowDesc::new(move || ui_builder());
    AppLauncher::with_window(main_window)
        .configure_env(|env, _| {
            env.set(druid::theme::FOREGROUND_LIGHT, druid::Color::rgb(0.05, 0.75, 0.75));
            env.set(druid::theme::FOREGROUND_DARK, druid::Color::rgb(0.1, 0.60, 0.55));

        })
        .use_simple_logger()
        .launch(UiData {
            controller: Arc::new(Mutex::new(controller)),
            is_playing: false,
            play_pos: 0.0,
        })
}

struct Anim <D, W> {
    w:W,
    _m:marker::PhantomData<D>,
}

impl<D, W> Anim <D, W> {
    fn new(w:W) -> Self {
        Self {
            w, 
            _m: Default::default()
        }
    }
}

impl<D, W>  Widget<D> for Anim<D, W> 
where W: Widget<D> {
    fn event(&mut self, ctx: &mut druid::EventCtx, event: &druid::Event, data: &mut D, env: &druid::Env) {
        ctx.request_anim_frame();
        self.w.event(ctx, event, data, env);
    }

    fn lifecycle(&mut self, ctx: &mut druid::LifeCycleCtx, event: &druid::LifeCycle, data: &D, env: &druid::Env) {
        self.w.lifecycle(ctx, event, data, env);
    }

    fn update(&mut self, ctx: &mut druid::UpdateCtx, old_data: &D, data: &D, env: &druid::Env) {
        self.w.update(ctx, old_data, data, env);
    }

    fn layout(
        &mut self,
        ctx: &mut druid::LayoutCtx,
        bc: &druid::BoxConstraints,
        data: &D,
        env: &druid::Env,
    ) -> druid::Size {
        self.w.layout(ctx, bc, data, env)   
    }

    fn paint(&mut self, ctx: &mut druid::PaintCtx, data: &D, env: &druid::Env) {
        self.w.paint(ctx, data, env);
    }
}
fn ui_builder() -> impl Widget<UiData> {
    let open = Button::new("Open")
        .on_click(move |_ctx, _data:&mut UiData, _env| {
            // todo: file dialog options
        });

    let seek_right = Button::new(">>")
        .on_click(|_, data:&mut UiData, _| {
            let controller = data.controller.try_lock(); 
            if controller.is_err() {
                println!("mutex was poisoned");
                return;
            }
            let mut controller = controller.unwrap();
            let file = controller.file.as_ref(); 
            if file.is_none() {
                println!("no file loaded");
                return;
            }
            let file = file.unwrap();
            let playhead = (controller.playhead() as f64) / file.sample_rate;
            controller.seek(playhead + 0.15);
        });

    let play = Button::new("|>")
        .on_click(move |_ctx, data: &mut UiData, _env| {
            data.is_playing = !data.is_playing;
            if let Ok(mut controller) = data.controller.try_lock() {
                if data.is_playing {
                    controller.play();
                } else {
                    controller.stop();
                }
            } else {
                println!("controller mutex was poisoned");
            }
        })
        .padding(5.0);

    let seek_left = Button::new("<<")
        .on_click(|_, data:&mut UiData, _| {
            let controller = data.controller.try_lock(); 
            if controller.is_err() {
                println!("mutex was poisoned");
                return;
            }
            let mut controller = controller.unwrap();
            controller.seek(0.0);
        });

    let waveform = Painter::new(move |ctx, data: &UiData, env|{
        let controller = if let Ok(c) = data.controller.try_lock() { c } else { return; };
        let file = if let Some(file) = controller.file.as_ref() { file } else { return };
        let size = ctx.size();
        let len = file.num_samples as f64;
        let step = 4 * (len / size.width).round() as usize;
        let (mut peak, mut avg) = file.plot(step, 0, 0, file.num_samples);
        let mut cursor = BezPath::new(); 
        let x = (controller.playhead() as f64) / len;
        cursor.move_to((x, 0.0)); 
        cursor.line_to((x, 1.0));

        peak.apply_affine(Affine::scale_non_uniform(size.width, size.height));
        avg.apply_affine(Affine::scale_non_uniform(size.width, size.height));
        cursor.apply_affine(Affine::scale_non_uniform(size.width, size.height));

        ctx.stroke(&peak, &env.get(druid::theme::FOREGROUND_DARK), 2.0);
        ctx.stroke(&avg, &env.get(druid::theme::FOREGROUND_LIGHT), 4.0);
        ctx.stroke(&cursor, &druid::Color::WHITE, 1.0);

    })
        .padding(5.0)
        .fix_height(100.0);

    Flex::column()
        .with_child(Flex::row()
            .with_child(open)
            .with_child(seek_left)
            .with_child(play)
            .with_child(seek_right))
        .with_child(
            Anim::new(
                Slider::new()
                    .lens(UiData::play_pos)
                    .expand_width()
                    .controller(PlayheadController)
                )
            )
        .with_child(Anim::new(waveform))
}
