use druid::{widget::{Button, Flex, Slider}};
use druid::{AppLauncher, PlatformError, Widget, WidgetExt, WindowDesc, RenderContext};
use basedrop::Collector;
use std::sync::{Arc, Mutex};
use druid::kurbo::{BezPath, Affine};
use std::marker;
use crate::{sample_player::SamplePlayerController};

struct PlayheadController;
impl<W: Widget<UiData>> druid::widget::Controller <UiData, W> for PlayheadController {
    fn event(&mut self, child: &mut W, ctx: &mut druid::EventCtx, event: &druid::Event, data: &mut UiData, env: &druid::Env) {
        if let druid::Event::AnimFrame(_) = event {
            if let Ok(controller) = data.controller.try_lock() {
                if let Some(file) = controller.file.as_ref() {
                    data.play_pos = (controller.playhead() as f64) / (file.num_samples as f64);
                }
            }
        }
        child.event(ctx, event, data, env);
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
                if ctx.is_active() {
                    let playhead = (file.num_samples as f64) / (file.sample_rate) * data.play_pos;
                    controller.seek(playhead);   
                }   
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
    peaks: Arc<Mutex<Vec<crate::audio_file::Peaks>>>,
}

pub fn run(_gc:Collector, controller:SamplePlayerController) -> Result<(), PlatformError> {
    let main_window = WindowDesc::new(move || ui_builder());
    AppLauncher::with_window(main_window)
        .use_simple_logger()
        .launch(UiData {
            peaks: Arc::new(Mutex::new(vec![
                controller.file.as_ref().unwrap().spectral_peaks(0),
                controller.file.as_ref().unwrap().spectral_peaks(1)
            ])),
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

    // let waveform = Painter::new(move |ctx, data: &UiData, env|{
    //     let bb = ctx.size().to_rect();
    //     ctx.fill(bb, &env.get(druid::theme::BACKGROUND_DARK));
    //     let controller = if let Ok(c) = data.controller.try_lock() { c } else { return; };
    //     let file = if let Some(file) = controller.file.as_ref() { file } else { return };
    //     let size = ctx.size();
    //     let len = file.num_samples as f64;
    //     let h = size.height;
    //     for ch in 0..file.num_channels {
    //         let (_, (path, gradient)) = &data.peaks[ch].peaks[3];
    //         let mut path = path.clone();
    //         path.apply_affine(Affine::translate((0.0, 0.5 + ch as f64)));
    //         path.apply_affine(Affine::scale_non_uniform(size.width, h / 2.0));
    //         ctx.stroke(&path, &druid::Color::BLACK, 1.0);
    //         ctx.fill(&path, gradient);
    //     }
    //     let x = (controller.playhead() as f64) / len;
    //     let mut cursor = BezPath::new(); 
    //     cursor.move_to((x, 0.0)); 
    //     cursor.line_to((x, 1.0));
    //     cursor.apply_affine(Affine::scale_non_uniform(size.width, size.height));
    //     ctx.stroke(&cursor, &druid::Color::WHITE, 1.0);
    // })
    let waveform = 
        WaveformView{}
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
        .with_child(Anim::new(waveform.padding(5.0)))
}

pub struct WaveformView;
impl Widget<UiData> for WaveformView {
    fn event(&mut self, 
        _ctx: &mut druid::EventCtx,
        event: &druid::Event,
        data: &mut UiData, 
        _env: &druid::Env) {
        if let druid::Event::Command(cmd) = event {
            // todo: avoid crashes here
            let mut guard = data.peaks.lock().expect("mutex poisoned");
            let peaks = &mut *guard;
            if let Some(size) = cmd.get::<druid::Size>(druid::Selector::new("SCALE_WAVEFORMS")) {
                println!("resizing waveforms: {:?}", size);
                let guard = data.controller.lock().expect("mutex poisoned");
                let file = guard.file.as_ref().expect("no file");
                for ch in 0..file.num_channels {
                    for (_, (path, _)) in &mut peaks[ch].peaks {
                        path.apply_affine(Affine::translate((0.0, 0.5 + ch as f64)));
                        path.apply_affine(Affine::scale_non_uniform(size.width, size.height / 2.0));
                    }
                }
            }
        }
    }

    fn update(
        &mut self,
        _ctx: &mut druid::UpdateCtx<'_, '_>,
        _old_data: &UiData,
        _data: &UiData,
        _env: &druid::Env
    ) {

    }

    fn lifecycle(
        &mut self,
        _ctx: &mut druid::LifeCycleCtx<'_, '_>,
        _event: &druid::LifeCycle,
        _data: &UiData,
        _env: &druid::Env
    ) {
        
    }

    fn layout(
        &mut self,
        ctx: &mut druid::LayoutCtx<'_, '_>,
        bc: &druid::BoxConstraints,
        _data: &UiData,
        _env: &druid::Env
    ) -> druid::Size {
        let size = bc.max();
        ctx.submit_command(druid::Command::new(
            druid::Selector::new("SCALE_WAVEFORMS"),
            size.clone(),
            druid::Target::Auto
        ));
        bc.max()
    }

    fn paint(&mut self, ctx: &mut druid::PaintCtx<'_, '_, '_>, data: &UiData, env: &druid::Env) {
        let bb = ctx.size().to_rect();
        ctx.fill(bb, &env.get(druid::theme::BACKGROUND_DARK));
        let controller = if let Ok(c) = data.controller.try_lock() { c } else { return; };
        let file = if let Some(file) = controller.file.as_ref() { file } else { return };
        let size = ctx.size();
        let len = file.num_samples as f64;
        let guard = data.peaks.try_lock().unwrap();
        let peaks = &*guard;
        for ch in 0..file.num_channels {
            let (_, (path, gradient)) = &peaks[ch].peaks[3];
            //let mut path = path.clone();
            //path.apply_affine(Affine::translate((0.0, 0.5 + ch as f64)));
            //path.apply_affine(Affine::scale_non_uniform(size.width, h / 2.0));
            ctx.stroke(&path, &druid::Color::BLACK, 1.0);
            ctx.fill(&path, gradient);
        }
        let x = (controller.playhead() as f64) / len;
        let mut cursor = BezPath::new(); 
        cursor.move_to((x, 0.0)); 
        cursor.line_to((x, 1.0));
        cursor.apply_affine(Affine::scale_non_uniform(size.width, size.height));
        ctx.stroke(&cursor, &druid::Color::WHITE, 1.0);
    }
}