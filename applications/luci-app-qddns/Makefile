# SPDX-License-Identifier: GPL-2.0-only

include $(TOPDIR)/rules.mk

LUCI_TITLE:=LuCI support for qddns
LUCI_DEPENDS:=+luci-base +qddns
LUCI_PKGARCH:=all

PKG_LICENSE:=GPL-2.0-only
PKG_MAINTAINER:=OpenAI

include $(TOPDIR)/feeds/luci/luci.mk

# call BuildPackage - OpenWrt buildroot signature
