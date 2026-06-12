from __future__ import annotations

from rich.console import Console, Group
from rich.panel import Panel
from rich.table import Table
from rich.text import Text
from rich.rule import Rule
from rich.columns import Columns

from src.core.game_state import GameState
from src.core.inhabitants import ROLE_ICONS
from src.events.event_base import Event, EventResult

console = Console()

TITLE_ART = r"""
                   |>>>                  |>>>
                   |                     |
               _  _|_  _             _  _|_  _
              |;|_|;|_|;|           |;|_|;|_|;|
              \\.    .  /           \\.    .  /
               \\:  .  /             \\:  .  /
                ||:   |               ||:   |
                ||:.  |               ||:.  |
                ||:  .|               ||:  .|
                ||:   |       __      ||:   |
                ||: , |      /  \     ||: , |
                ||:   |     | () |    ||:   |
                ||: . |      \__/     ||: . |
              __||_   |________________||_   |__
"""


def _morale_bar(morale: int) -> Text:
    filled = morale // 5
    color = "green" if morale >= 60 else "yellow" if morale >= 30 else "red"
    bar = Text()
    bar.append("█" * filled, style=color)
    bar.append("░" * (20 - filled), style="grey37")
    bar.append(f" {morale}/100", style=color)
    return bar


def show_title():
    console.clear()
    console.print(Text(TITLE_ART, style="bold cyan"))
    console.print(Rule("[bold yellow]ADVENTURE FORTRESS[/bold yellow]"))
    console.print(
        "[dim]Survive 30 days. Keep morale above zero. Every choice matters.[/dim]",
        justify="center",
    )
    console.print()


def show_status(game_state: GameState):
    f = game_state.fortress
    r = game_state.resources

    info = Table.grid(padding=(0, 2))
    info.add_column(style="bold")
    info.add_column()
    info.add_row("Morale", _morale_bar(f.morale))
    info.add_row("Defense", f"[bold cyan]{f.defense}[/bold cyan]")
    info.add_row(
        "Resources",
        f"🍞 [yellow]{r.food}[/yellow]  💰 [gold1]{r.gold}[/gold1]  "
        f"🪵 [green4]{r.wood}[/green4]  🪨 [grey62]{r.stone}[/grey62]",
    )
    if f.upgrades:
        info.add_row("Built", "[magenta]" + ", ".join(str(u) for u in f.upgrades) + "[/magenta]")

    console.print(
        Panel(
            info,
            title=f"[bold]Day {f.day}/30 — {f.name}[/bold]",
            border_style="cyan",
        )
    )


def show_inhabitants(game_state: GameState):
    alive = game_state.inhabitants.get_alive()
    if not alive:
        console.print(Panel("[dim]The fortress stands empty.[/dim]", title="Inhabitants", border_style="blue"))
        return

    table = Table(show_header=True, header_style="bold blue", expand=False)
    table.add_column("Name", min_width=10)
    table.add_column("Role", min_width=12)
    table.add_column("Health", justify="right")
    table.add_column("Morale", justify="right")
    table.add_column("Traits")

    for i in alive:
        health_style = "green" if i.health >= 60 else "yellow" if i.health >= 30 else "red"
        morale_style = "green" if i.morale >= 60 else "yellow" if i.morale >= 30 else "red"
        table.add_row(
            i.name,
            f"{ROLE_ICONS[i.role]} {i.role}",
            f"[{health_style}]{i.health}[/{health_style}]",
            f"[{morale_style}]{i.morale}[/{morale_style}]",
            "[italic dim]" + ", ".join(i.traits) + "[/italic dim]" if i.traits else "",
        )

    console.print(Panel(table, title=f"Inhabitants ({len(alive)}/{game_state.fortress.max_population})", border_style="blue"))


def show_history(history: list[str]):
    if not history:
        return
    lines = Group(*[Text(f"• {line}", style="dim") for line in history[-3:]])
    console.print(Panel(lines, title="Recent Days", border_style="grey37"))


def show_event(event: Event):
    console.print()
    console.print(Rule(style="yellow"))
    console.print(
        Panel(
            f"[italic]{event.description}[/italic]",
            title=f"[bold yellow]⚜ {event.name}[/bold yellow]",
            border_style="yellow",
        )
    )


def show_choices(event: Event, available: list[bool]):
    for idx, choice in enumerate(event.choices):
        cost_str = ""
        if choice.cost:
            cost_str = " [dim](costs " + ", ".join(f"{v} {k}" for k, v in choice.cost.items()) + ")[/dim]"
        if available[idx]:
            console.print(f"  [bold cyan]{idx + 1}.[/bold cyan] [bold]{choice.label}[/bold]{cost_str}")
            console.print(f"     [dim]{choice.description}[/dim]")
        else:
            console.print(f"  [grey37]{idx + 1}. {choice.label}{cost_str} — can't afford[/grey37]")
    console.print("  [grey50]s. Save & quit[/grey50]")


def show_result(result: EventResult):
    body = Group(*[Text(f"  {line}") for line in result.lines]) if result.lines else Text("  Nothing comes of it.")
    console.print(
        Panel(
            body,
            title=f"[bold]{result.choice_label}[/bold]",
            border_style="green",
        )
    )


def show_daily_log(lines: list[str]):
    for line in lines:
        console.print(f"  [dim cyan]» {line}[/dim cyan]")


def show_game_over(game_state: GameState, victory: bool):
    f = game_state.fortress
    stats = Table.grid(padding=(0, 2))
    stats.add_column(style="bold")
    stats.add_column()
    stats.add_row("Days survived", str(f.day - 1 if not victory else 30))
    stats.add_row("Events faced", str(game_state.events_resolved))
    stats.add_row("Inhabitants alive", str(game_state.inhabitants.count_alive()))
    stats.add_row("Inhabitants lost", str(game_state.inhabitants.count_dead()))
    stats.add_row("Upgrades built", ", ".join(str(u) for u in f.upgrades) or "none")
    stats.add_row("Run seed", str(game_state.run_seed))

    if victory:
        if f.morale >= 70:
            headline = "TRIUMPH! The fortress thrives, its name sung across the realm."
        elif f.morale >= 35:
            headline = "VICTORY. Battered but unbroken, the fortress endures."
        else:
            headline = "SURVIVAL. By a thread, the fortress saw the thirtieth dawn."
        style = "bold green"
    else:
        headline = "THE FORTRESS HAS FALLEN. Its halls stand silent."
        style = "bold red"

    console.print()
    console.print(Panel(Group(Text(headline, style=style), Text(), stats), title="Final Reckoning", border_style=style))


def prompt_choice(num_choices: int, available: list[bool]) -> int | str:
    """Returns a 0-based choice index, or 's' for save-and-quit."""
    while True:
        raw = console.input("[bold]Choose[/bold] ❯ ").strip().lower()
        if raw == "s":
            return "s"
        if raw.isdigit():
            idx = int(raw) - 1
            if 0 <= idx < num_choices and available[idx]:
                return idx
        console.print("[red]  Invalid choice — pick an available option.[/red]")
