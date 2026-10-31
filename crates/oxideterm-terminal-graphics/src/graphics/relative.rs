impl GraphicsIngress {
    fn parent_placement(
        &self,
        image: TerminalImageId,
        placement: u32,
    ) -> Option<&TerminalImagePlacement> {
        self.kitty_placements
            .iter()
            .rev()
            .find(|p| p.id == image && p.kitty.as_ref().is_some_and(|k| k.id == placement))
    }

    fn kitty_parent(
        &self,
        image: TerminalImageId,
        placement: u32,
        params: &HashMap<String, String>,
    ) -> Result<Option<(TerminalImageId, u32, i32, i32)>, GraphicsError> {
        let Some(parent) = parse_u32(params.get("P")).filter(|p| *p != 0) else {
            return Ok(None);
        };
        if params.get("U").is_some_and(|u| u == "1") {
            return Err(GraphicsError::InvalidCommand);
        }
        let parent = self
            .kitty_wire_ids
            .get(&(self.alternate_screen, parent))
            .copied()
            .ok_or(GraphicsError::ParentNotFound)?;
        let parent_placement = parse_u32(params.get("Q")).unwrap_or(0);
        let mut next = Some((parent, parent_placement));
        let mut depth = 0;
        while let Some((id, p)) = next {
            if placement != 0 && (id, p) == (image, placement) {
                return Err(GraphicsError::PlacementCycle);
            }
            depth += 1;
            if depth > 8 {
                return Err(GraphicsError::PlacementDepth);
            }
            let ancestor = self
                .parent_placement(id, p)
                .ok_or(GraphicsError::ParentNotFound)?;
            next = ancestor
                .kitty
                .as_ref()
                .and_then(|k| k.parent)
                .map(|(id, p, _, _)| (id, p));
        }
        let horizontal = params
            .get("H")
            .and_then(|p| p.parse::<i32>().ok())
            .unwrap_or(0);
        let vertical = params
            .get("V")
            .and_then(|p| p.parse::<i32>().ok())
            .unwrap_or(0);
        Ok(Some((parent, parent_placement, horizontal, vertical)))
    }

    fn relative_origin(
        &self,
        placement: &TerminalImagePlacement,
        depth: usize,
    ) -> Option<(i32, i64)> {
        if depth > 8 {
            return None;
        }
        let kitty = placement.kitty.as_ref()?;
        if let Some((image, id, horizontal, vertical)) = kitty.parent {
            let parent = self.parent_placement(image, id)?;
            let (line, col) = self.relative_origin(parent, depth + 1)?;
            Some((
                line.saturating_add(vertical),
                col.saturating_add(i64::from(horizontal)),
            ))
        } else if kitty.virtual_placement {
            let mut locations = self.placeholder_rendered.values().filter(|p| {
                p.id == placement.id && p.kitty.as_ref().is_some_and(|k| k.id == kitty.id)
            });
            let first = locations.next()?;
            Some(
                locations.fold((first.line, first.col as i64), |(line, col), p| {
                    (line.min(p.line), col.min(p.col as i64))
                }),
            )
        } else {
            Some((placement.line, placement.col as i64 + kitty.column_offset))
        }
    }

    fn resolve_placement(&self, placement: &mut TerminalImagePlacement) {
        if placement.kitty.as_ref().is_none_or(|k| k.parent.is_none()) {
            return;
        }
        let position = self.relative_origin(placement, 0);
        let kitty = placement.kitty.as_mut().unwrap();
        kitty.hidden = position.is_none();
        if let Some((line, col)) = position {
            placement.line = line;
            placement.col = col.max(0) as usize;
            kitty.column_offset = col.min(0);
        }
    }

    fn refresh_relatives(&mut self, events: &mut Vec<TerminalGraphicsEvent>) {
        for index in 0..self.kitty_placements.len() {
            if self.kitty_placements[index]
                .kitty
                .as_ref()
                .is_none_or(|k| k.parent.is_none())
            {
                continue;
            }
            let mut next = self.kitty_placements[index].clone();
            self.resolve_placement(&mut next);
            if next != self.kitty_placements[index] {
                self.kitty_placements[index] = next.clone();
                events.push(TerminalGraphicsEvent::Place(next));
            }
        }
    }

    fn cascade_missing_parents(&mut self, events: &mut Vec<TerminalGraphicsEvent>) {
        loop {
            let missing =
                self.kitty_placements
                    .iter()
                    .find(|p| {
                        p.kitty.as_ref().and_then(|k| k.parent).is_some_and(
                            |(id, placement, _, _)| self.parent_placement(id, placement).is_none(),
                        )
                    })
                    .cloned();
            let Some(placement) = missing else { break };
            let serial = placement.kitty.as_ref().unwrap().serial;
            self.kitty_placements
                .retain(|p| p.kitty.as_ref().unwrap().serial != serial);
            if let Some(grid) = &self.grid_events {
                grid.set_anchor(placement.alternate_screen, serial, None);
            }
            events.push(TerminalGraphicsEvent::DeletePlacement { serial });
            if self.kitty_placements.iter().all(|p| p.id != placement.id) {
                self.account_image_event(&TerminalGraphicsEvent::Delete {
                    id: Some(placement.id),
                });
                events.push(TerminalGraphicsEvent::Delete {
                    id: Some(placement.id),
                });
            }
        }
    }
}
