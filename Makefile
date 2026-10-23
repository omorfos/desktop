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

PROGRAMS = omorfos omorfosctl omorfos-panel omorfos-launcher omorfos-notifications \
	omorfos-screenshot omorfos-clipboard omorfos-recording omorfos-session omorfos-ask \
	omorfos-wallpaper omorfos-lock xdg-desktop-portal-omorfos
PAM_SERVICES = omorfos-lock omorfos-lock-fingerprint omorfos-lock-key
BUILT = target/$(PROFILE)

.PHONY: all nightly build install uninstall

all: build

nightly:
	$(MAKE) build PROFILE=nightly

build:
	cargo build --profile $(PROFILE) --locked $(addprefix -p ,$(PROGRAMS))

install:
	@for program in $(PROGRAMS); do \
		[ -x "$(BUILT)/$$program" ] || { \
			echo "$(BUILT)/$$program is not built: run make$(if $(filter nightly,$(PROFILE)), nightly,) first, as yourself" >&2; \
			exit 1; \
		}; \
	done
	install -d "$(DESTDIR)$(BINDIR)"
	install -m755 $(addprefix $(BUILT)/,$(PROGRAMS)) "$(DESTDIR)$(BINDIR)/"
	install -Dm644 packaging/omorfos.desktop "$(DESTDIR)$(DATADIR)/wayland-sessions/omorfos.desktop"
	install -Dm644 packaging/omorfos.portal "$(DESTDIR)$(DATADIR)/xdg-desktop-portal/portals/omorfos.portal"
	install -Dm644 packaging/omorfos-portals.conf "$(DESTDIR)$(DATADIR)/xdg-desktop-portal/omorfos-portals.conf"
	install -Dm644 packaging/omorfos-session.target "$(DESTDIR)$(SYSTEMD_USER_DIR)/omorfos-session.target"
	install -Dm644 README.md "$(DESTDIR)$(DOC_DIR)/README.md"
	install -Dm644 docs/CONFIGURATION.md "$(DESTDIR)$(DOC_DIR)/CONFIGURATION.md"
	install -Dm644 crates/omorfos-config/src/default.css "$(DESTDIR)$(DOC_DIR)/theme.css"
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
	rm -f $(addprefix "$(DESTDIR)$(BINDIR)/,$(addsuffix ",$(PROGRAMS)))
	rm -f "$(DESTDIR)$(DATADIR)/wayland-sessions/omorfos.desktop"
	rm -f "$(DESTDIR)$(DATADIR)/xdg-desktop-portal/portals/omorfos.portal"
	rm -f "$(DESTDIR)$(DATADIR)/xdg-desktop-portal/omorfos-portals.conf"
	rm -f "$(DESTDIR)$(SYSTEMD_USER_DIR)/omorfos-session.target"
	rm -f "$(DESTDIR)$(DOC_DIR)/README.md" "$(DESTDIR)$(DOC_DIR)/CONFIGURATION.md" "$(DESTDIR)$(DOC_DIR)/theme.css"
	@echo "The PAM services in $(DESTDIR)$(PAM_DIR) are left, as an administrator may have changed them: $(PAM_SERVICES)"
