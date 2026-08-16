CARGO ?= cargo
INSTALL ?= install
PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
CARGO_TARGET_DIR ?= target

BIN := spotter
RELEASE_BIN := $(CARGO_TARGET_DIR)/release/$(BIN)

.PHONY: all build install

all: build

build:
	$(CARGO) build --release --bin $(BIN)

install: build
	$(INSTALL) -d "$(DESTDIR)$(BINDIR)"
	$(INSTALL) -m 0755 "$(RELEASE_BIN)" "$(DESTDIR)$(BINDIR)/$(BIN)"
