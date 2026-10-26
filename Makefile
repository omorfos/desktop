# Builds and installs omorfos from source.
#
#   make                          the programs, optimised as a release is
#   make nightly                  the same, built in seconds after a change (see AGENTS.md)
#   sudo make install             what was built, into /usr/local and the system's places
#   sudo make install PROFILE=nightly
#   sudo make uninstall
#
# `install` copies what was built and builds nothing, so that nothing is
# built as root: build first, as yourself. PREFIX, DESTDIR and the
# directories below may be set on the command line, as a package does.

PROFILE ?= release
PREFIX ?= /usr/local
DESTDIR ?=
BINDIR ?= $(PREFIX)/bin
# Where login screens, the portal and systemd look, whatever the prefix.
DATADIR ?= /usr/share
SYSTEMD_USER_DIR ?= /usr/lib/systemd/user
PAM_DIR ?= /etc/pam.d
DOC_DIR ?= $(PREFIX)/share/doc/omorfos
# Where the icon and the wallpapers go, which programs find through
# XDG_DATA_DIRS, /usr/local/share among them.
SHARE_DIR ?= $(PREFIX)/share

PROGRAMS = omorfos omorfos-hold omorfosctl omorfos-panel omorfos-launcher omorfos-notifications \
	omorfos-screenshot omorfos-clipboard omorfos-recording omorfos-session omorfos-ask \
	omorfos-wallpaper omorfos-lock xdg-desktop-portal-omorfos
# What is installed: each program, and those built beside another, as
# omorfos-pinentry is beside omorfos-ask.
INSTALLED = $(PROGRAMS) omorfos-pinentry
PAM_SERVICES = omorfos-lock omorfos-lock-fingerprint omorfos-lock-key
BUILT = target/$(PROFILE)
# The pictures of the brand, drawn by omorfos-brand as omorfos is built.
PICTURES = $(BUILT)/share
# The wallpapers it draws, each dark and light, as omorfos_brand::DESIGNS
# has them.
WALLPAPERS = prism spill horizon facets ground

.PHONY: all nightly build install uninstall

all: build

nightly:
	$(MAKE) build PROFILE=nightly

build:
	cargo build --profile $(PROFILE) --locked $(addprefix -p ,$(PROGRAMS)) -p omorfos-brand
	$(MAKE) $(PICTURES).drawn

# Drawn again only when the program that draws them has changed.
$(PICTURES).drawn: $(BUILT)/omorfos-brand
	rm -rf "$(PICTURES)"
	"$(BUILT)/omorfos-brand" "$(PICTURES)"
	touch "$@"

install:
	@for program in $(INSTALLED); do \
		[ -x "$(BUILT)/$$program" ] || { \
			echo "$(BUILT)/$$program is not built: run make$(if $(filter nightly,$(PROFILE)), nightly,) first, as yourself" >&2; \
			exit 1; \
		}; \
	done
	@[ -e "$(PICTURES).drawn" ] || { echo "$(PICTURES) is not drawn: run make$(if $(filter nightly,$(PROFILE)), nightly,) first, as yourself" >&2; exit 1; }
	install -d "$(DESTDIR)$(BINDIR)"
	install -m755 $(addprefix $(BUILT)/,$(INSTALLED)) "$(DESTDIR)$(BINDIR)/"
	install -Dm644 packaging/omorfos.desktop "$(DESTDIR)$(DATADIR)/wayland-sessions/omorfos.desktop"
	install -Dm644 packaging/omorfos.portal "$(DESTDIR)$(DATADIR)/xdg-desktop-portal/portals/omorfos.portal"
	install -Dm644 packaging/omorfos-portals.conf "$(DESTDIR)$(DATADIR)/xdg-desktop-portal/omorfos-portals.conf"
	install -Dm644 packaging/omorfos-session.target "$(DESTDIR)$(SYSTEMD_USER_DIR)/omorfos-session.target"
	install -Dm644 README.md "$(DESTDIR)$(DOC_DIR)/README.md"
	install -Dm644 docs/CONFIGURATION.md "$(DESTDIR)$(DOC_DIR)/CONFIGURATION.md"
	install -Dm644 crates/omorfos-config/src/default.css "$(DESTDIR)$(DOC_DIR)/theme.css"
	install -Dm644 LICENSE-pictures "$(DESTDIR)$(DOC_DIR)/LICENSE-pictures"
	cd "$(PICTURES)" && find icons backgrounds -type f -exec install -Dm644 {} "$(DESTDIR)$(SHARE_DIR)/{}" \;
	@# The PAM services are the system's once installed, and an
	@# administrator's changes to them are kept.
	@for service in $(PAM_SERVICES); do \
		if [ -e "$(DESTDIR)$(PAM_DIR)/$$service" ]; then \
			echo "kept $(DESTDIR)$(PAM_DIR)/$$service, which is already there"; \
		else \
			echo "install -Dm644 packaging/pam/$$service $(DESTDIR)$(PAM_DIR)/$$service"; \
			install -Dm644 "packaging/pam/$$service" "$(DESTDIR)$(PAM_DIR)/$$service"; \
		fi; \
	done

uninstall:
	rm -f $(addprefix "$(DESTDIR)$(BINDIR)/,$(addsuffix ",$(INSTALLED)))
	rm -f "$(DESTDIR)$(DATADIR)/wayland-sessions/omorfos.desktop"
	rm -f "$(DESTDIR)$(DATADIR)/xdg-desktop-portal/portals/omorfos.portal"
	rm -f "$(DESTDIR)$(DATADIR)/xdg-desktop-portal/omorfos-portals.conf"
	rm -f "$(DESTDIR)$(SYSTEMD_USER_DIR)/omorfos-session.target"
	rm -f "$(DESTDIR)$(DOC_DIR)/README.md" "$(DESTDIR)$(DOC_DIR)/CONFIGURATION.md" "$(DESTDIR)$(DOC_DIR)/theme.css" "$(DESTDIR)$(DOC_DIR)/LICENSE-pictures"
	rm -f "$(DESTDIR)$(SHARE_DIR)"/icons/hicolor/*/apps/omorfos.png "$(DESTDIR)$(SHARE_DIR)"/icons/hicolor/*/apps/omorfos.svg "$(DESTDIR)$(SHARE_DIR)"/icons/hicolor/*/apps/omorfos-symbolic.svg
	rm -f $(foreach w,$(WALLPAPERS),"$(DESTDIR)$(SHARE_DIR)/backgrounds/omorfos/$(w)-dark.png" "$(DESTDIR)$(SHARE_DIR)/backgrounds/omorfos/$(w)-light.png")
	@# Left if there is anything else in it, as a wallpaper someone put there.
	-rmdir "$(DESTDIR)$(SHARE_DIR)/backgrounds/omorfos" 2>/dev/null
	@echo "The PAM services in $(DESTDIR)$(PAM_DIR) are left, as an administrator may have changed them: $(PAM_SERVICES)"
