from __future__ import annotations
import os

from src.core.game_state import GameState
from src.core.inhabitants import Role, generate_inhabitant
from src.events.event_engine import EventEngine
from src.events.event_pool import load_events
from src.ui import display

SAVE_PATH = "save.json"


def new_game() -> GameState:
    name = display.console.input("[bold]Name your fortress[/bold] ❯ ").strip() or "Greyhold"
    gs = GameState()
    gs.fortress.name = name
    gs.resources.apply_delta({"food": 40, "gold": 30, "wood": 20, "stone": 10})
    for role in (Role.GUARD, Role.FARMER, Role.FARMER, Role.HEALER):
        gs.inhabitants.add(generate_inhabitant(role, gs.rng))
    return gs


def start_screen() -> GameState:
    display.show_title()
    if os.path.exists(SAVE_PATH):
        raw = display.console.input(
            "[bold]A saved game exists.[/bold] [cyan](c)[/cyan]ontinue or [cyan](n)[/cyan]ew game? ❯ "
        ).strip().lower()
        if raw != "n":
            try:
                return GameState.load(SAVE_PATH)
            except (KeyError, ValueError, OSError):
                display.console.print("[red]Save file is corrupted — starting fresh.[/red]")
        os.remove(SAVE_PATH)
    return new_game()


def run(game_state: GameState):
    engine = EventEngine(events=load_events(), game_state=game_state)
    history: list[str] = []

    while not game_state.is_game_over() and not game_state.is_victory():
        display.console.print()
        display.show_status(game_state)
        display.show_inhabitants(game_state)
        display.show_history(history)

        event = engine.roll(day=game_state.fortress.day)
        if event is not None:
            display.show_event(event)
            available = [engine.choice_available(c) for c in event.choices]
            display.show_choices(event, available)

            picked = display.prompt_choice(len(event.choices), available)
            if picked == "s":
                game_state.save(SAVE_PATH)
                display.console.print(f"[green]Saved to {SAVE_PATH}. Until next time.[/green]")
                return

            result = engine.resolve(event, picked)
            display.show_result(result)
            history.append(f"Day {game_state.fortress.day}: {event.name} — {result.choice_label}")
        else:
            display.console.print("[dim]A quiet day passes.[/dim]")
            history.append(f"Day {game_state.fortress.day}: a quiet day")

        daily_log = game_state.apply_daily_effects()
        display.show_daily_log(daily_log)
        game_state.fortress.advance_day()
        game_state.save(SAVE_PATH)

    display.show_game_over(game_state, victory=game_state.is_victory())
    if os.path.exists(SAVE_PATH):
        os.remove(SAVE_PATH)


def main():
    try:
        run(start_screen())
    except (KeyboardInterrupt, EOFError):
        display.console.print("\n[dim]Farewell.[/dim]")


if __name__ == "__main__":
    main()
