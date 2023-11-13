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
pub struct App<TMiddlewares>
{
    state: AppRunState,
    tick_count: TickCount,
    pub middlewares: TMiddlewares,
}

impl<'a, TMiddlewares> App<TMiddlewares>
where
    TMiddlewares: Middlewares
{
    pub fn new(middlewares: TMiddlewares) -> Self { Self
    {
        state: AppRunState::NotRunning,
        tick_count: TickCount(0),
        middlewares: middlewares,
    }}

    pub fn run_once(&mut self)
    {
        self.tick_count.0 += 1;

        match self.state
        {
            AppRunState::NotRunning => return,
            AppRunState::StartingUp =>
            {
                // todo: measure startup/shutdown time, abort if too slow?
                match self.middlewares.startup()
                {
                    CompletionState::Completed =>
                    {
                        self.state = AppRunState::Running;
                        eprintln!("{} App looping", log_time());
                    },
                    CompletionState::InProgress => (),
                }
            }
            AppRunState::ShuttingDown =>
            {
                match self.middlewares.shutdown()
                {
                    CompletionState::Completed =>
                    {
                        self.state = AppRunState::NotRunning;
                        eprintln!("{} App shut down", log_time());
                    },
                    CompletionState::InProgress => (),
                }
            }
            AppRunState::Running =>
            {
                match self.middlewares.run()
                {
                    CompletionState::Completed =>
                    {
                        self.state = AppRunState::ShuttingDown;
                        eprintln!("{} App shutting down", log_time());
                    },
                    CompletionState::InProgress => (),
                }
            }
        }
    }

    pub fn run(&mut self)
    {
        assert_eq!(AppRunState::NotRunning, self.state);
        self.state = AppRunState::StartingUp;

        eprint!("{} App starting up with args", log_time());
        for arg in std::env::args()
        {
            eprint!(" {}", arg);
        }
        eprintln!();

        while self.state != AppRunState::NotRunning
        {
            self.run_once();
        }
    }
}