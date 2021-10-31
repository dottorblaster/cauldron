build:
	arch-meson _build && ninja -C _build

install:
	meson install -C _build

default: build
