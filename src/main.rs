use std::borrow::Cow;
use futures::{future::FutureExt, StreamExt};
use ratatui::{
    prelude::*,
    backend::CrosstermBackend,
    buffer::Buffer,
    crossterm::{
        event::{self, Event, KeyCode},
        ExecutableCommand,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::Rect,
};
use solitaire_base::index::{
    Slot as SolitaireSlot,
    Location as SolitaireLocation,
};

#[derive(Clone)]
struct Board {
    board: solitaire_base::Board,
}

fn key_to_slot(key: KeyCode) -> Result<SolitaireSlot, ()> {
    Ok(match key {
        KeyCode::Char('a') => SolitaireSlot::Spare(0),
        KeyCode::Char('b') => SolitaireSlot::Spare(1),
        KeyCode::Char('c') => SolitaireSlot::Spare(2),
        KeyCode::Char('1') => SolitaireSlot::Tray(0),
        KeyCode::Char('2') => SolitaireSlot::Tray(1),
        KeyCode::Char('3') => SolitaireSlot::Tray(2),
        KeyCode::Char('4') => SolitaireSlot::Tray(3),
        KeyCode::Char('5') => SolitaireSlot::Tray(4),
        KeyCode::Char('6') => SolitaireSlot::Tray(5),
        KeyCode::Char('7') => SolitaireSlot::Tray(6),
        KeyCode::Char('8') => SolitaireSlot::Tray(7),
        _ => return Err(()),
    })
}

enum BoardState {
    View,
    CollectDragon,
    SemiPickup(u8),
    Pickup(SolitaireLocation),
}

impl Board {
    fn new() -> Self {
        Self {
            board: solitaire_base::Board::new_random(),
        }
    }
}

impl StatefulWidget for Board {
    type State = BoardState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        assert!(area.width >= 25);

        let (sx, sy) = (area.x, area.y);

        let card_style_normal = Style::reset();
        let card_style_semi_selected = Style::reset().on_gray();
        let card_style_selected = Style::reset().white().on_black();
        let card_style_flower = Style::reset().red();

        trait Colorize {
            fn colorize(&self, card: &solitaire_base::card::Card) -> Style;
        }

        impl Colorize for Style {
            fn colorize(&self, card: &solitaire_base::card::Card) -> Style {
                match card {
                    solitaire_base::card::Card::Number(solitaire_base::card::NumberCard::Bamboo, _) => self.green(),
                    solitaire_base::card::Card::Number(solitaire_base::card::NumberCard::Characters, _) => self.black(),
                    solitaire_base::card::Card::Number(solitaire_base::card::NumberCard::Coin, _) => self.red(),
                    solitaire_base::card::Card::Dragon(solitaire_base::card::DragonCard::Green) => self.green(),
                    solitaire_base::card::Card::Dragon(solitaire_base::card::DragonCard::White) => self.black(),
                    solitaire_base::card::Card::Dragon(solitaire_base::card::DragonCard::Red) => self.red(),
                    solitaire_base::card::Card::Flower => self.magenta(),
                }
            }
        }

        //Top Line
        buf.set_string(sx, sy, "+--+--+--+-----+--+--+--+", Style::reset());
        //Spares
        (0..3).for_each(|i| {
            buf.set_string(sx + i * 3, sy + 1, "|", card_style_normal);
            buf.set_string(sx + i * 3 + 1, sy + 1,
                            match self.board.get(SolitaireSlot::Spare(i as u8)).next() {
                                Some(card) => Cow::Owned(format!("{card}")),
                                None if self.board.is_spare_collected(i as u8) => Cow::Borrowed("CO"),
                                None => Cow::Borrowed("  "),
                            },
                            match state {
                                BoardState::Pickup(SolitaireLocation::Spare(n)) if *n == i as u8 => card_style_selected,
                                BoardState::CollectDragon => card_style_semi_selected,
                                _ => if let Some(card) = self.board.get(SolitaireSlot::Spare(i as u8)).next() { card_style_normal.colorize(card) } else { card_style_normal },
                            });
        });
        //Flower
        buf.set_string(sx + 9, sy + 1, "| ", card_style_normal);
        buf.set_string(sx + 11, sy + 1, if self.board.flower() { "F L" } else { "   " }, card_style_flower);
        //Out
        buf.set_string(sx + 14, sy + 1, format!(" |G{}|B{}|R{}|", self.board.out().bamboo, self.board.out().characters, self.board.out().coin), card_style_normal);
        //Separator
        buf.set_string(sx, sy + 2, "+--+--+--+-----+--+--+--+", card_style_normal);
        //Tray
        let height = (0..8).map(|x| self.board.get(SolitaireSlot::Tray(x)).count()).max().unwrap_or(0);
        for i in 0..height {
            for j in 0..8 {
                let card = self.board.get(SolitaireSlot::Tray(j)).nth(i);
                buf.set_string(sx + j as u16 * 3, sy + 3 + i as u16, " ", card_style_normal);
                buf.set_string(sx + j as u16 * 3 + 1, sy + 3 + i as u16,
                               if let Some(c) = card { Cow::Owned(format!("{c}")) } else { Cow::Borrowed("  ") },
                               if let Some(card) = card { match state {
                                   BoardState::SemiPickup(n) if *n == j => { card_style_semi_selected }
                                   BoardState::Pickup(SolitaireLocation::Tray(n, m)) if *n == j && i + 2 > *m as usize => { card_style_selected }
                                   _ => card_style_normal.colorize(card)
                               }} else { card_style_normal })
            }
        }
    }
}

fn change_board_state(board: &mut Board, state: &mut BoardState, info: &mut Option<String>, key: KeyCode) {
    if info.is_some() { *info = None; }
    match state {
        BoardState::View => {
            match key {
                KeyCode::Char('a') => *state = BoardState::Pickup(SolitaireLocation::Spare(0)),
                KeyCode::Char('b') => *state = BoardState::Pickup(SolitaireLocation::Spare(1)),
                KeyCode::Char('c') => *state = BoardState::Pickup(SolitaireLocation::Spare(2)),
                KeyCode::Char('1') => *state = BoardState::SemiPickup(0),
                KeyCode::Char('2') => *state = BoardState::SemiPickup(1),
                KeyCode::Char('3') => *state = BoardState::SemiPickup(2),
                KeyCode::Char('4') => *state = BoardState::SemiPickup(3),
                KeyCode::Char('5') => *state = BoardState::SemiPickup(4),
                KeyCode::Char('6') => *state = BoardState::SemiPickup(5),
                KeyCode::Char('7') => *state = BoardState::SemiPickup(6),
                KeyCode::Char('8') => *state = BoardState::SemiPickup(7),
                KeyCode::Char('d') => *state = BoardState::CollectDragon,
                _ => {}
            }
        }
        BoardState::CollectDragon => {
            if key == KeyCode::Esc {
                *state = BoardState::View;
            } else {
                let color = match key {
                    KeyCode::Char('g') => solitaire_base::card::DragonCard::Green,
                    KeyCode::Char('w') => solitaire_base::card::DragonCard::White,
                    KeyCode::Char('r') => solitaire_base::card::DragonCard::Red,
                    _ => return,
                };
                if board.board.move_cards(solitaire_base::move_action::MoveAction::CollectDragon(color)) {
                    *state = BoardState::View;
                } else {
                    *info = Some("Cannot collect dragon".to_string());
                }
            }
        }
        BoardState::SemiPickup(index) => {
            if key == KeyCode::Esc {
                *state = BoardState::View;
            } else if let KeyCode::Char(c) = key {
                if let Some(n) = c.to_digit(10) {
                    let index_th_stack_len = board.board.get(SolitaireSlot::Tray(*index)).count();
                    if n == 0 || n as usize > index_th_stack_len { return; }
                    for i in n as usize..index_th_stack_len {
                        if !board.board[SolitaireLocation::Tray(*index, i as u8)]
                            .can_stack_onto(&board.board[SolitaireLocation::Tray(*index, i as u8 - 1)]) {
                            *info = Some("Not a valid stack".into());
                            return;
                        }
                    }
                    *state = BoardState::Pickup(SolitaireLocation::Tray(*index, n as u8));
                }
            }
        }
        BoardState::Pickup(location) => {
            if key == KeyCode::Esc {
                *state = BoardState::View;
            } else {
                let target_slot = if let Ok(slot) = key_to_slot(key) { slot } else { return; };
                let source_card = match location {
                    SolitaireLocation::Spare(index) => board.board.get(SolitaireSlot::Spare(*index)).next(),
                    SolitaireLocation::Tray(x, y) => board.board.get(SolitaireSlot::Tray(*x)).nth((*y - 1) as usize),
                }.unwrap();
                //Check if the move is valid
                if !board.board.appendable(target_slot, source_card) {
                    *info = Some("Cannot stack onto that".to_string());
                    return;
                }
                //Move the card(delete from source and add to target)
                let cards = match location {
                    SolitaireLocation::Spare(index) => vec![board.board.pop(SolitaireSlot::Spare(*index)).unwrap()],
                    SolitaireLocation::Tray(x, y) => {
                        (*y as usize - 1..board.board.get(SolitaireSlot::Tray(*x)).count())
                            .map(|_| board.board.pop(SolitaireSlot::Tray(*x)).unwrap())
                            .collect::<Vec<solitaire_base::card::Card>>().iter().rev().copied().collect()
                    }
                };
                for card in cards.iter() {
                    board.board.push(target_slot, *card);
                }
                board.board.simplify();
                *state = BoardState::View;
            }
        }
    }
}

fn draw(frame: &mut Frame, board: &Board, board_state: &mut BoardState, info: &Option<String>) {
    let vertical_layout = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]);
    let [board_area, status_line] = vertical_layout.areas(Rect::new(0, 0, 25, 17));
    
    frame.render_stateful_widget(board.clone(), board_area, board_state);
    
    frame.render_widget(if let Some(info) = info {
        Line::from(info.clone()).on_red()
    } else {
        let cards = solitaire_base::index::ALL_SLOTS.iter().map(|slot| board.board.get(*slot).count()).sum::<usize>();
        Line::from(if cards == 0 { Cow::Borrowed("Congratulations!") } else { Cow::Owned(format!("{cards}/40 cards left")) }).right_aligned().on_gray()
    }, status_line);
}

#[tokio::main]
async fn main() -> std::io::Result<()>{
    enable_raw_mode()?;
    std::io::stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
    let mut event_stream = event::EventStream::new();

    let panic_fn = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |x| {
        crossterm::execute!(std::io::stderr(), LeaveAlternateScreen)
            .unwrap();
        disable_raw_mode().unwrap();
        panic_fn(x);
    }));

    let mut board = Board::new();
    let mut board_state = BoardState::View;
    let mut info = None::<String>;

    loop {
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                terminal.draw(|frame| {
                    draw(frame, &board, &mut board_state, &info);
                })?;
            }
            Some(Ok(event)) = event_stream.next().fuse() => {
                let key;
                if let Event::Key(k) = event { key = k; } else { continue; }
                if key == KeyCode::Char('q').into() {
                   break;
                } else if key == KeyCode::Char('n').into() {
                    board = Board::new();
                } else {
                    change_board_state(&mut board, &mut board_state, &mut info, key.code);
                }
                terminal.draw(|frame| {
                    draw(frame, &board, &mut board_state, &info);
                })?;
            },
        }
    }

    disable_raw_mode()?;
    std::io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}