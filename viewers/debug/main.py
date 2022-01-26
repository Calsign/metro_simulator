import time
import os
import math
import functools

import argh

import engine

# suppress banner
if True:
    os.environ["PYGAME_HIDE_SUPPORT_PROMPT"] = "hide"
    import pygame
    import pygame_gui


WINDOW_SIZE = (1920, 1080)
FRAMERATE = 60
DEFAULT_SCALE = 4

DEFAULT_CONFIG = "config/debug.toml"


class Colors:
    BACKGROUND = (0, 0, 0)
    TILE_SIDES = (200, 200, 200)
    TILE_HIDDEN = (100, 100, 100)
    TEXT = (255, 255, 255)


class Controls:
    PAN_MOUSE_BUTTON = 1
    SELECT_MOUSE_BUTTON = 3


def main(config=DEFAULT_CONFIG, load_file=None):
    if load_file is None:
        state = engine.State(engine.Config(config))
    else:
        state = engine.State.load(load_file)

    min_scale = min(WINDOW_SIZE) / state.width / 2
    max_scale = 100

    # current scale; pixels per tile
    scale = min_scale
    # current translation; pixels
    tx = WINDOW_SIZE[0] / 2 - state.width * scale / 2
    ty = WINDOW_SIZE[1] / 2 - state.width * scale / 2

    pygame.init()
    display = pygame.display.set_mode(WINDOW_SIZE)

    font = pygame.freetype.SysFont(pygame.freetype.get_default_font(), 20)

    gui = pygame_gui.UIManager(WINDOW_SIZE)

    detail_panel = pygame_gui.elements.UIPanel(
        relative_rect=pygame.Rect((50, 50), (400, 200)),
        manager=gui,
        starting_layer_height=1,
    )

    detail_panel_address = pygame_gui.elements.UITextEntryLine(
        relative_rect=pygame.Rect((10, 10), (370, 0)),
        manager=gui,
        container=detail_panel,
    )

    detail_panel_json = pygame_gui.elements.UITextEntryLine(
        relative_rect=pygame.Rect((10, 50), (370, 0)),
        manager=gui,
        container=detail_panel,
    )

    detail_panel_json_edit = pygame_gui.elements.UITextEntryLine(
        relative_rect=pygame.Rect((10, 90), (370, 0)),
        manager=gui,
        container=detail_panel,
    )

    detail_panel_split = pygame_gui.elements.UIButton(
        relative_rect=pygame.Rect((10, 130), (100, 30)),
        manager=gui,
        container=detail_panel,
        text="Split",
    )

    diagnostics_panel = pygame_gui.elements.UIPanel(
        relative_rect=pygame.Rect((50, 270), (400, 200)),
        manager=gui,
        starting_layer_height=1,
    )

    diagnostics_panel_rendered = pygame_gui.elements.UITextEntryLine(
        relative_rect=pygame.Rect((10, 10), (370, 0)),
        manager=gui,
        container=diagnostics_panel,
    )
    diagnostics_panel_rendered.disable()

    diagnostics_panel_framerate = pygame_gui.elements.UITextEntryLine(
        relative_rect=pygame.Rect((10, 50), (370, 0)),
        manager=gui,
        container=diagnostics_panel,
    )
    diagnostics_panel_framerate.disable()

    selected_tile = None
    detail_panel.disable()

    def select_tile(address):
        nonlocal selected_tile
        selected_tile = address

        if selected_tile is None:
            detail_panel.disable()
            detail_panel_address.set_text("")
            detail_panel_json.set_text("")
            detail_panel_json_edit.set_text("")
        else:
            detail_panel.enable()
            detail_panel_address.set_text(str(address.get()))
            detail_panel_address.disable()
            json = state.get_leaf_json(address)
            detail_panel_json.set_text(json)
            detail_panel_json.disable()
            detail_panel_json_edit.set_text(json)
            if len(address.get()) < state.max_depth:
                detail_panel_split.enable()
            else:
                detail_panel_split.disable()

    def split_tile(address):
        if len(address.get()) < state.max_depth:
            state.split(
                address,
                engine.BranchState(),
                engine.LeafState(),
                engine.LeafState(),
                engine.LeafState(),
                engine.LeafState(),
            )
            if selected_tile is not None and selected_tile.get() == address.get():
                select_tile(None)

    @functools.cache
    def text_map(text):
        return font.render(text, Colors.TEXT)

    def screen_coords(m):
        mx, my = m
        return (mx * scale + tx, my * scale + ty)

    def model_coords(s):
        sx, sy = s
        return (round((sx - tx) / scale), round((sy - ty) / scale))

    def get_address_under_cursor():
        mx, my = model_coords(pygame.mouse.get_pos())
        if mx > 0 and mx < state.width and my > 0 and my < state.width:
            return state.get_address(mx, my)
        else:
            return None

    def visit_branch(branch, data):
        if data.width * scale >= 10:
            return True
        else:
            # don't draw things that are too small to see
            x, y = screen_coords((data.x, data.y))
            w = data.width * scale
            pygame.draw.rect(display, Colors.TILE_HIDDEN, pygame.Rect(x, y, w, w))

    def visit_leaf(leaf, data):
        x, y = screen_coords((data.x, data.y))
        w = data.width * scale

        if selected_tile is not None and selected_tile.get() == data.address.get():
            width = 5
        else:
            width = 1

        pygame.draw.lines(
            display,
            Colors.TILE_SIDES,
            True,
            ((x, y), (x + w, y), (x + w, y + w), (x, y + w)),
            width,
        )

        text, rect = text_map(leaf.name)
        if w > rect.width * 1.5:
            display.blit(
                text, (x + w / 2 - rect.width / 2, y + w / 2 - rect.height / 2)
            )

        nonlocal rendered
        rendered += 1

    def handle_event(event):
        nonlocal tx, ty
        if event.type == pygame.QUIT:
            raise KeyboardInterrupt
        elif event.type == pygame.MOUSEBUTTONDOWN:
            if event.button == Controls.SELECT_MOUSE_BUTTON:
                address = get_address_under_cursor()
                select_tile(address)
        elif event.type == pygame.MOUSEMOTION:
            if event.buttons[Controls.PAN_MOUSE_BUTTON - 1]:
                tx += event.rel[0]
                ty += event.rel[1]
        elif event.type == pygame.MOUSEWHEEL:
            nonlocal scale
            mouse_x, mouse_y = pygame.mouse.get_pos()
            new_scale = max(min(scale * 1.2 ** event.y, max_scale), min_scale)

            # Zoom centered on the mouse
            # Invariant: (mouse_x - tx) / scale = (mouse_x - tx') / scale'
            # Solved: tx' = (mouse_x * scale - mouse_x * scale' + tx * scale') / scale
            tx = (mouse_x * scale - mouse_x * new_scale + tx * new_scale) / scale
            ty = (mouse_y * scale - mouse_y * new_scale + ty * new_scale) / scale

            scale = new_scale
        elif event.type == pygame.KEYDOWN:
            # detect ctrl+s
            if event.key == ord("s") and event.mod in [
                pygame.KMOD_LCTRL,
                pygame.KMOD_RCTRL,
            ]:
                state.save(
                    "/tmp/metro_simulator_{}.json".format(math.floor(time.time()))
                )
            elif event.key == ord("t") and event.mod in [
                pygame.KMOD_LCTRL,
                pygame.KMOD_RCTRL,
            ]:
                if selected_tile is not None:
                    split_tile(selected_tile)
                else:
                    under_cursor = get_address_under_cursor()
                    if under_cursor is not None:
                        split_tile(under_cursor)
            elif event.key == pygame.K_ESCAPE:
                select_tile(None)
        elif event.type == pygame.USEREVENT:
            if event.user_type == pygame_gui.UI_TEXT_ENTRY_FINISHED:
                if event.ui_element == detail_panel_json_edit:
                    try:
                        state.set_leaf_json(
                            selected_tile, detail_panel_json_edit.get_text()
                        )
                        # update with changed value
                        select_tile(selected_tile)
                    except Exception as e:
                        print(e)
            if event.user_type == pygame_gui.UI_BUTTON_PRESSED:
                if event.ui_element == detail_panel_split:
                    split_tile(selected_tile)

    clock = pygame.time.Clock()

    while True:
        time_delta = clock.tick(FRAMERATE) / 1000.0

        for event in pygame.event.get():
            gui.process_events(event)
            handle_event(event)

        gui.update(time_delta)

        x1, y1 = model_coords((0, 0))
        x2, y2 = model_coords(display.get_size())

        rendered = 0

        display.fill(Colors.BACKGROUND)
        state.visit_rect(
            visit_branch, visit_leaf, max(x1, 0), max(x2, 0), max(y1, 0), max(y2, 0)
        )

        diagnostics_panel_rendered.set_text("Rendered: {}".format(rendered))
        diagnostics_panel_framerate.set_text(
            "Frame rate: {}".format(round(clock.get_fps(), 1))
        )

        gui.draw_ui(display)

        pygame.display.update()


if __name__ == "__main__":
    try:
        argh.dispatch_command(main)
    except KeyboardInterrupt:
        pass
