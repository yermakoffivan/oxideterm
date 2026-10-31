pub(crate) struct TerminalGraphicsState {
    images: HashMap<TerminalImageId, Arc<TerminalImageData>>,
    next_image_version: u64,
    placements: Vec<TerminalImagePlacement>,
    alternate_screen: bool,
}

impl Default for TerminalGraphicsState {
    fn default() -> Self {
        Self {
            images: HashMap::new(),
            next_image_version: 1,
            placements: Vec::new(),
            alternate_screen: false,
        }
    }
}

impl TerminalGraphicsState {
    pub(crate) fn handle_event(&mut self, event: TerminalGraphicsEvent) -> Option<Vec<u8>> {
        match event {
            TerminalGraphicsEvent::Screen { alternate } => {
                self.alternate_screen = alternate;
                None
            }
            TerminalGraphicsEvent::MoveCursor { .. } => None,
            TerminalGraphicsEvent::ImageReady(mut image) => {
                self.placements.retain(|placement| placement.id != image.id);
                image.version = self.allocate_image_version();
                self.images.insert(image.id, Arc::new(image));
                None
            }
            TerminalGraphicsEvent::ImageUpdated(mut image) => {
                image.version = self.allocate_image_version();
                self.images.insert(image.id, Arc::new(image));
                None
            }
            TerminalGraphicsEvent::Place(placement) => {
                if let Some(kitty) = &placement.kitty {
                    self.placements.retain(|existing| {
                        existing
                            .kitty
                            .as_ref()
                            .is_none_or(|old| old.serial != kitty.serial)
                    });
                } else {
                    self.placements
                        .retain(|existing| existing.id != placement.id);
                }
                self.placements.push(placement);
                None
            }
            TerminalGraphicsEvent::DeletePlacement { serial } => {
                self.placements
                    .retain(|p| p.kitty.as_ref().is_none_or(|k| k.serial != serial));
                None
            }
            TerminalGraphicsEvent::Delete { id } => {
                if let Some(id) = id {
                    self.remove_image(id);
                    self.placements.retain(|placement| placement.id != id);
                } else {
                    self.clear();
                }
                None
            }
            TerminalGraphicsEvent::DeletePlacements { id } => {
                self.placements.retain(|placement| {
                    placement.protocol != TerminalImageProtocol::Kitty
                        || id.is_some_and(|id| placement.id != id)
                });
                None
            }
            TerminalGraphicsEvent::Batch(events) => {
                let mut responses = Vec::new();
                for event in events {
                    if let Some(response) = self.handle_event(event) {
                        responses.extend(response);
                    }
                }
                (!responses.is_empty()).then_some(responses)
            }
            TerminalGraphicsEvent::Respond(bytes) => Some(bytes),
            TerminalGraphicsEvent::Error(error) => {
                tracing::debug!(%error, "terminal graphics protocol error");
                None
            }
        }
    }

    pub(crate) fn clear(&mut self) {
        self.images.clear();
        self.placements.clear();
    }

    fn allocate_image_version(&mut self) -> u64 {
        // A global content revision avoids retaining a tombstone for every image id ever seen.
        let version = self.next_image_version;
        self.next_image_version = self.next_image_version.wrapping_add(1).max(1);
        version
    }

    fn visible_images(&self, display_offset: usize, rows: usize) -> Vec<TerminalImageSnapshot> {
        self.placements
            .iter()
            .filter_map(|placement| {
                if placement.alternate_screen != self.alternate_screen {
                    return None;
                }
                if placement
                    .kitty
                    .as_ref()
                    .is_some_and(|k| k.virtual_placement || k.hidden)
                {
                    return None;
                }
                let signed_row =
                    i64::from(placement.line) + display_offset.min(i64::MAX as usize) as i64;
                let visible_rows = if placement
                    .kitty
                    .as_ref()
                    .is_some_and(|k| k.unicode.is_some())
                {
                    1
                } else {
                    placement.rows
                };
                if signed_row >= rows as i64
                    || signed_row.saturating_add(visible_rows.min(i64::MAX as usize) as i64) <= 0
                {
                    return None;
                }
                let row = signed_row.max(0) as usize;
                Some(TerminalImageSnapshot {
                    id: placement.id,
                    protocol: placement.protocol,
                    row,
                    col: placement.col,
                    cols: placement.cols,
                    rows: placement.rows,
                    pixel_width: placement.pixel_width,
                    pixel_height: placement.pixel_height,
                    source_x: placement.source_x,
                    source_y: placement.source_y,
                    source_width: placement.source_width,
                    source_height: placement.source_height,
                    z_index: placement.z_index,
                    placeholder: placement.placeholder,
                    kitty: placement.kitty.clone(),
                    clipped_rows: signed_row.saturating_neg().max(0) as usize,
                    version: self
                        .images
                        .get(&placement.id)
                        .map(|image| image.version)
                        .unwrap_or_default(),
                    data: self.images.get(&placement.id).cloned(),
                })
            })
            .collect()
    }

    fn remove_image(&mut self, id: TerminalImageId) {
        self.images.remove(&id);
    }
}

pub(crate) fn apply_graphics_terminal_event<T: EventListener>(
    term: &mut Term<T>,
    event: &TerminalGraphicsEvent,
) {
    use alacritty_terminal::vte::ansi::Handler;
    if let TerminalGraphicsEvent::MoveCursor { cols, rows } = event {
        term.move_forward(*cols);
        term.move_down(*rows);
    }
}

pub(crate) fn graphics_cursor_from_term<T: EventListener>(
    term: &Term<T>,
    size: TerminalSize,
) -> GraphicsCursor {
    term.capture_graphics_placeholders();
    let content = term.renderable_content();
    let line = content.cursor.point.line.0;
    GraphicsCursor {
        line,
        row: viewport_row_for_grid_line(line, content.display_offset).unwrap_or_default(),
        col: content.cursor.point.column.0,
        cols: size.cols,
        rows: size.rows,
        cell_width: size.cell_width,
        cell_height: size.cell_height,
    }
}
