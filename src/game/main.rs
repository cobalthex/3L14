use std::hash::Hash;
use std::thread::sleep;
use glam::Vec2;
use game_3l14::{engine::{*, middlewares::{clock::*, window::*, input::*}}};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, WindowCanvas};
use sdl2::ttf::{Font, Sdl2TtfContext};

use game_3l14::engine::state_logic;

fn main()
{
    println!("Starting 3L14");

    let mut clock = Clock::new();
    let sdl = sdl2::init().unwrap();
    let mut sdl_events = sdl.event_pump().unwrap();
    let mut sdl_video = sdl.video().unwrap();
    let ttf_context = sdl2::ttf::init().unwrap();

    // middlewares
    let mut windows = Windows::new(&sdl_video);
    let mut input = Input::default();

    let debug_render_config = DebugRenderConfig
    {
        font: ttf_context.load_font(r#"C:\Windows\Fonts\Consola.ttf"#, 14).unwrap(),
    };

    let min_frame_time = std::time::Duration::from_secs_f32(1.0 / 150.0);

    // const MAX_APP_THREADS: usize = 4;
    // const MAX_APP_JOB_QUEUE_DEPTH: usize = 256; // must be power of 2
    // let job_system = JobSystem::new(MAX_APP_THREADS, MAX_APP_JOB_QUEUE_DEPTH)
    //     .expect("Failed to create app job system"); // todo: pass in config

    let mut frame_number = FrameNumber(0);
    let mut render_frame = RenderSdl2DFrame::new(frame_number, &debug_render_config);
    'main: loop
    {
        let mut completion = CompletionState::InProgress;

        frame_number.increment();
        let frame_start_time = clock.tick(); // todo: cap fps

        input.pre_update();

        for event in sdl_events.poll_iter()
        {
            match event
            {
                sdl2::event::Event::Quit {..} =>
                    {
                        completion |= CompletionState::Completed;
                    },
                _ => input.handle_event(event, frame_start_time.current_time),
            }
        }

        if input.keyboard().get_key_down(KeyCode::Q).is_some() &&
           input.keyboard().has_keymod(KeyMods::CTRL)
        {
            completion = CompletionState::Completed;
        }

        if let Some(delta) = min_frame_time.checked_sub(frame_start_time.delta_time)
        {
            sleep(delta)
        }

        render_frame.reset(frame_number);
        render_frame.debug_text(format!("{:3.1}", 1.0 / frame_start_time.delta_time.as_secs_f32()), Vec2::new(10.0, 10.0));

        render_frame.debug_text(format!("{:#?}", input), Vec2::new(30.0 ,30.0));

        // for (i, key) in input.keyboard_state.pressed_keys.iter().enumerate()
        // {
        //     render_frame.debug_text(format!("{:?}", key), Vec2::new(20.0, 40.0 + (i as f32) * 16.0));
        // }

        let main_wnd = windows.main_window_mut();
        main_wnd.clear();
        render_frame.render(main_wnd);
        main_wnd.present();

        if completion == CompletionState::Completed
        {
            break 'main
        }
    }

    println!("Exiting 3L14");
}

struct DebugRenderConfig<'ttf>
{
    pub font: Font<'ttf, 'static>,
}

struct RenderText
{
    pub text: String,
    pub position: Vec2,
}

struct RenderSdl2DFrame<'f>
{
    frame_number: FrameNumber,
    debug_config: &'f DebugRenderConfig<'f>,

    texts: Vec<RenderText>,
}

impl<'f> RenderSdl2DFrame<'f>
{
    pub fn new(frame_number: FrameNumber, debug_config: &'f DebugRenderConfig) -> Self
    {
        Self
        {
            frame_number,
            debug_config,

            texts: Vec::new(),
        }
    }
    pub fn reset(&mut self, frame_number: FrameNumber)
    {
        self.frame_number = frame_number;
        self.texts.clear();
    }

    pub fn debug_text(&mut self, text: String, position: Vec2)
    {
        self.texts.push(RenderText { text, position });
    }

    pub fn render(&mut self, canvas: &mut WindowCanvas)
    {
        let texture_creator = canvas.texture_creator();
        self.texts.iter().for_each(|text|
        {
            let pr = self.debug_config.font.render(text.text.as_str());
            let surf = pr.blended_wrapped(Color::RGB(255, 255, 255), 512).unwrap();
            let surf_rect = surf.rect();
            let texture = texture_creator.create_texture_from_surface(surf).unwrap();

            canvas.copy(&texture,
                        surf_rect,
                        Rect::new(text.position.x as i32, text.position.y as i32, surf_rect.width(), surf_rect.height()))
                .unwrap();
        })
    }
}
