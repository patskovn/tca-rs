# The Composable Architecture

> Note: that is currently a prototype developed for my own learning Rust sake and couple of terminal-UI apps.

The Composable Architecture (TCA, for short) is a library for building applications in a consistent and understandable way, with composition, testing, and ergonomics in mind. 

This library is heavily inspired by [TCA implementation for Swift language](https://github.com/pointfreeco/swift-composable-architecture). 

## Usage

There are 3 main components. First, define your `State`, `Action` and `Feature`.
```rust
#[derive(Clone, Eq)]
struct State {
    counter: i32,
}

#[derive(Debug)]
enum Action {
    UpdateCounter(i32),
}

struct Feature {}
```

Then, we need to implement `Reducer` trait for `Feature`:

```rust
impl tca::Reducer<State, Action> for Feature {
    fn reduce(state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::UpdateCounter(counter) => {
                state.counter = counter;

                Effect::none()
            }
        }
    }
}
```

Now you can create new `Store` to use in you main event loop. Let's use [ratatui](https://ratatui.rs/) crate as our UI library.

```rust
#[tokio::main]
fn main() -> anyhow::Result<()> {
    let terminal = /* Creating and preparing terminal here */;

    // Instantiate root store.
    let store = tca::Store::new::<Feature>(State::default());

    // Stream of redraw requests. Called each time `State` is changed by `Reducer`.
    let mut redraw_events = store.observe();

    // Stream of terminal events (e.g. key presses).
    let mut terminal_events = crossterm::event::EventStream::new();

    // Our main event loop.
    loop {
        let crossterm_event = terminal_events.next().fuse();
        let redraw_event = redraw_events.recv().fuse();
        tokio::select! {
            maybe_redraw = redraw_event => {
                match maybe_redraw {
                    Ok(()) => {
                        terminal.draw(|f| ui(f, &state))?;
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        continue;
                    }    
                }
            }
            maybe_event = crossterm_event => {
                match maybe_event {
                    Some(Ok(evt)) => store.send(Action::Event(evt)),
                    Some(Err(err)) => return Err(err.into()),
                    None => continue,
                }
            }
        }
    }

    Ok(())
}
```
