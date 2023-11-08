use chrono::{format::{DelayedFormat, StrftimeItems}, Local};
use super::*;

#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub enum AppRunState
{
    #[default]
    NotRunning,
    StartingUp,
    ShuttingDown,
    Running,
}

#[derive(Debug, Default)]
pub struct AppContext
{
    state: AppRunState,
    tick_count: TickCount,

    // shared, global data
}
#[allow(dead_code)] // todo: remove
impl AppContext
{
    pub fn state(&self) -> AppRunState { self.state }
    pub fn tick_count(&self) -> TickCount { self.tick_count }
}

#[derive(Debug, Default)]
pub struct App<TMiddlewares: Middlewares>
{
    pub context: AppContext,
    pub middlewares: TMiddlewares,
}

impl<'a, TMiddlewares> App<TMiddlewares>
where
    TMiddlewares: Middlewares
{
    pub fn new(middlewares: TMiddlewares) -> Self { Self
    {
        context: Default::default(),
        middlewares: middlewares,
    }}

    pub fn run_once(&mut self)
    {
        self.context.tick_count.0 += 1;

        match self.context.state
        {
            AppRunState::NotRunning => return,
            AppRunState::StartingUp =>
            {
                // todo: measure startup/shutdown time, abort if too slow?
                match self.middlewares.startup(&mut self.context)
                {
                    CompletionState::Completed =>
                    {
                        self.context.state = AppRunState::Running;
                        eprintln!("{} App looping", log_time());
                    },
                    CompletionState::InProgress => (),
                }
            }
            AppRunState::ShuttingDown =>
            {
                match self.middlewares.shutdown(&mut self.context)
                {
                    CompletionState::Completed =>
                    {
                        self.context.state = AppRunState::NotRunning;
                        eprintln!("{} App shut down", log_time());
                    },
                    CompletionState::InProgress => (),
                }
            }
            AppRunState::Running =>
            {
                match self.middlewares.run(&mut self.context)
                {
                    CompletionState::Completed =>
                    {
                        self.context.state = AppRunState::ShuttingDown;
                        eprintln!("{} App shutting down", log_time());
                    },
                    CompletionState::InProgress => (),
                }
            }
        }
    }

    pub fn run(&mut self)
    {
        assert_eq!(AppRunState::NotRunning, self.context.state);
        self.context.state = AppRunState::StartingUp;

        eprintln!("{} App starting up", log_time());

        self.middlewares.each(|m| eprintln!("{} Starting up middleware '{}'", log_time(), m.name()));

        while self.context.state != AppRunState::NotRunning
        {
            self.run_once();
        }
    }
}