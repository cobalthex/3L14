use egui::{Pos2, Sense, Stroke, Vec2};

pub struct Sparkline<const N_ENTRIES: usize>
{
    entries: [f32; N_ENTRIES],
    start: usize,
    count: usize,

    sum: f32,
    min: f32,
    max: f32,
}
impl<const N_ENTRIES: usize> Sparkline<N_ENTRIES>
{
    pub fn new() -> Self
    {
        Self
        {
            entries: [0.0; N_ENTRIES],
            start: 0,
            count: 0,

            sum: f32::default(),
            min: f32::default(),
            max: f32::default(),
        }
    }

    pub fn average_f32(&self) -> f32
    {
        let sum: f32 = self.sum.into();
        sum / (self.count as f32)
    }

    pub fn clear(&mut self)
    {
        self.start = 0;
        self.count = 0;

        self.sum = f32::default();
        self.min = f32::default();
        self.max = f32::default();
    }

    pub fn add(&mut self, value: f32)
    {
        if self.count < N_ENTRIES
        {
            debug_assert_eq!(self.start, 0);
            self.entries[self.count] = value;
            self.count += 1;
            self.sum += value;
        }
        else
        {
            debug_assert!(self.count == N_ENTRIES);
            self.sum += -self.entries[self.start] + value;
            self.entries[self.start] = value;
            self.start = (self.start + 1) % N_ENTRIES;
        }

        self.calc_minmax();
    }

    fn calc_minmax(&mut self)
    {
        if self.count < 1
        {
            return
        }

        self.min = self.entries[0];
        self.max = self.min;

        for i in 1..self.count
        {
            let val = self.entries[i];
            if val < self.min { self.min = val }
            if val > self.max { self.max = val }
        }
    }
}
impl<const N_ENTRIES: usize> Drop for Sparkline<N_ENTRIES>
{
    fn drop(&mut self) { self.clear() }
}
impl<const N_ENTRIES: usize> egui::Widget for &Sparkline<N_ENTRIES>
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response
    {
        let desired_size = Vec2::new(200.0, 30.0);
        let senses = Sense
        {
            click: false,
            drag: false,
            focusable: false,
        };

        if self.count < 2
        {
            return ui.allocate_response(desired_size, senses);
        }

        let (response, painter) = ui.allocate_painter(desired_size, senses);

        let clip = painter.clip_rect();
        let seg_width = clip.width() / (N_ENTRIES as f32);
        let range = (self.max - self.min).max(30.0); // todo: this should decay slowly
        let get_pos = |i, val|
        {
            let norm = ((val - self.min) / (range * 2.0)) + 0.5;
            Pos2
            {
                x: clip.min.x + seg_width * (i as f32),
                y: clip.min.y + (norm * (clip.max.y - clip.min.y))
            }
        };

        let stroke = Stroke
        {
            width: 1.0,
            color: ui.style().visuals.text_color(),
        };

        let mut last_pos = get_pos(0, self.entries[self.start]);
        for i in 1..(self.count)
        {
            let current_pos = get_pos(i, self.entries[(i + self.start) % N_ENTRIES]);
            painter.line_segment([last_pos, current_pos], stroke);
            last_pos = current_pos;
        }

        response
    }
}
