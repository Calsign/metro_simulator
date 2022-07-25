// I did not have much success trying to use egui's plotting support, and in any case I don't need
// anything that complicated. So I have decided to write my own instead.

pub struct Entry {
    pub value: f32,
    pub extra: Option<f32>,
}

impl Entry {
    pub fn peak(&self) -> f32 {
        self.value + self.extra.unwrap_or(0.0).max(0.0)
    }
}

impl From<f32> for Entry {
    fn from(value: f32) -> Self {
        Self { value, extra: None }
    }
}

impl From<(f32, f32)> for Entry {
    fn from((value, extra): (f32, f32)) -> Self {
        Self {
            value,
            extra: Some(extra),
        }
    }
}

impl From<(f32, Option<f32>)> for Entry {
    fn from((value, extra): (f32, Option<f32>)) -> Self {
        Self { value, extra }
    }
}

pub struct Chart<E: Into<Entry>>
where
    E: Copy,
{
    data: Vec<E>,
    pub max_entry: Option<f32>,
    pub rounded_max_entry: f32,
    labeler: Option<Box<dyn Fn(usize, E) -> String>>,
}

impl<E: Into<Entry>> Chart<E>
where
    E: Copy,
{
    pub fn new(data: Vec<E>) -> Self {
        let max_entry = data
            .iter()
            .map(|e| E::into(*e).peak())
            .max_by(|x, y| x.partial_cmp(y).unwrap());
        let rounded_max_entry = max_entry
            .map(|max_entry| {
                if max_entry > 0.0 {
                    4.0_f32.powf(max_entry.log(4.0).ceil())
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        Self {
            data,
            max_entry,
            rounded_max_entry,
            labeler: None,
        }
    }

    pub fn with_labels<F>(&mut self, labeler: F)
    where
        F: Fn(usize, E) -> String + 'static,
    {
        self.labeler = Some(Box::new(labeler));
    }

    fn bar_height(&self, height: f32, value: f32) -> f32 {
        if self.rounded_max_entry > 0.0 {
            height * value / self.rounded_max_entry
        } else {
            0.0
        }
    }
}

impl<E: Into<Entry>> egui::widgets::Widget for Chart<E>
where
    E: Copy,
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (w, _) = ui.available_size().into();
        let (response, painter) = ui.allocate_painter((w, w / 2.0).into(), egui::Sense::hover());

        let rect = painter.clip_rect();
        let (x1, y1) = rect.min.into();
        let (x2, y2) = rect.max.into();

        // background
        painter.rect_filled(rect, egui::Rounding::none(), egui::Color32::BLACK);

        if self.max_entry.is_some() {
            let width = (x2 - x1) / self.data.len() as f32;
            let height = y2 - y1;

            for (i, entry) in self.data.iter().enumerate() {
                let entry: Entry = (*entry).into();

                let bar_height = self.bar_height(height, entry.value);
                let r = egui::Rect::from_min_max(
                    painter.round_pos_to_pixels((x1 + width * i as f32, y2 - bar_height).into()),
                    painter.round_pos_to_pixels((x1 + width * (i + 1) as f32, y2).into()),
                );
                painter.rect_filled(r, egui::Rounding::none(), egui::Color32::LIGHT_GRAY);

                // display bar stacked on top of the first
                if let Some(extra) = entry.extra {
                    let extra_height = self.bar_height(height, extra);
                    let r = egui::Rect::from_min_max(
                        painter.round_pos_to_pixels(
                            (
                                x1 + width * i as f32,
                                y2 - bar_height - extra_height.max(0.0),
                            )
                                .into(),
                        ),
                        painter.round_pos_to_pixels(
                            (
                                x1 + width * (i + 1) as f32,
                                y2 - bar_height - extra_height.min(0.0),
                            )
                                .into(),
                        ),
                    );
                    let c = if extra > 0.0 {
                        // "bad"
                        egui::Color32::DARK_RED
                    } else {
                        // "good"
                        egui::Color32::DARK_GREEN
                    };
                    painter.rect_filled(r, egui::Rounding::none(), c);
                }
            }
        }

        if let Some(labeler) = self.labeler {
            if let Some(hover_pos) = response.hover_pos() {
                if !self.data.is_empty() && hover_pos.x > x1 && hover_pos.x < x2 {
                    let width = (x2 - x1) / self.data.len() as f32;
                    let index = ((hover_pos.x - x1) / width).floor() as usize;
                    let label = labeler(index, self.data[index]);
                    egui::show_tooltip_at_pointer(ui.ctx(), egui::Id::new(&label), |ui| {
                        ui.label(label);
                    });
                }
            }
        }

        response
    }
}
