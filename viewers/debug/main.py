
import time
import os
import math

import engine

# suppress banner
if True:
    os.environ['PYGAME_HIDE_SUPPORT_PROMPT'] = "hide"
    import pygame


WINDOW_SIZE = (1920, 1080)
FRAMERATE = 60
DEFAULT_SCALE = 4


def main():
    config = engine.Config("config/debug.toml")
    state = engine.State(config)

    # current scale; pixels per tile
    scale = min(WINDOW_SIZE) / state.width
    # current translation; pixels
    tx = WINDOW_SIZE[0] / 2 - state.width * scale / 2
    ty = WINDOW_SIZE[1] / 2 - state.width * scale / 2

    pygame.init()

    display = pygame.display.set_mode(WINDOW_SIZE)

    def screen_coords(m):
        mx, my = m
        return (mx * scale + tx, my * scale + ty)

    def model_coords(s):
        sx, sy = s
        return (round((sx - tx) / scale), round((sy - ty) / scale))

    def visit_branch(branch, data):
        if data.width * scale >= 10:
            return True
        else:
            # don't draw things that are too small to see
            x, y = screen_coords((data.x, data.y))
            w = data.width * scale
            pygame.draw.rect(display, (150, 150, 150), pygame.Rect(x, y, w, w))

    def visit_leaf(branch, data):
        x, y = screen_coords((data.x, data.y))
        w = data.width * scale
        pygame.draw.lines(display, (255, 255, 255), True,
                          ((x, y), (x+w, y), (x+w, y+w), (x, y+w)))

    def handle_input():
        for event in pygame.event.get():
            if event.type == pygame.MOUSEBUTTONDOWN:
                if event.button == 1:
                    mx, my = model_coords(pygame.mouse.get_pos())
                    if mx > 0 and mx < state.width and my > 0 and my < state.width:
                        address = state.get_address(mx, my)
                        if len(address.get()) < state.max_depth:
                            state.split(address, engine.BranchState(), engine.LeafState(),
                                        engine.LeafState(), engine.LeafState(), engine.LeafState())
            elif event.type == pygame.MOUSEMOTION:
                if event.buttons[2]:
                    nonlocal tx, ty
                    tx += event.rel[0]
                    ty += event.rel[1]
            elif event.type == pygame.MOUSEWHEEL:
                nonlocal scale
                mouse_x, mouse_y = pygame.mouse.get_pos()
                new_scale = scale * 1.2**event.y

                # Zoom centered on the mouse
                # Invariant: (mouse_x - tx) / scale = (mouse_x - tx') / scale'
                # Solved: tx' = (mouse_x * scale - mouse_x * scale' + tx * scale') / scale
                tx = (mouse_x * scale - mouse_x *
                      new_scale + tx * new_scale) / scale
                ty = (mouse_y * scale - mouse_y *
                      new_scale + ty * new_scale) / scale

                scale = new_scale

    while True:
        handle_input()

        display.fill((100, 100, 100))
        state.visit(visit_branch, visit_leaf)

        pygame.display.update()
        time.sleep(1 / FRAMERATE)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        pass
