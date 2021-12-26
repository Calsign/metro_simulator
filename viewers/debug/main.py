
import time

import pygame

import engine


def main():
    pygame.init()

    print(engine.foobar())

    display = pygame.display.set_mode((800, 600))

    while True:
        pygame.display.update()
        time.sleep(100)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        pass
