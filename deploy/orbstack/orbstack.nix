# Vendored from the /etc/nixos/orbstack.nix that OrbStack 2.2.1 generated at
# machine creation (2026-07-04), reformatted to satisfy this repo's nix
# lints. OrbStack overwrites its copy on upgrades; this one is owned by the
# flake. Re-diff against /etc/nixos/orbstack.nix in the VM after OrbStack
# upgrades and fold changes in as commits.
{
  lib,
  config,
  ...
}: {
  # Add OrbStack CLI tools to PATH
  environment = {
    shellInit = ''
      . /opt/orbstack-guest/etc/profile-early

      # add your customizations here

      . /opt/orbstack-guest/etc/profile-late
    '';

    # Disable systemd-resolved (OrbStack manages resolv.conf)
    etc."resolv.conf".source = "/opt/orbstack-guest/etc/resolv.conf";
  };

  # Enable documentation
  documentation = {
    man.enable = true;
    doc.enable = true;
    info.enable = true;
  };

  services = {
    # Disable systemd-resolved
    resolved.enable = false;
    # Disable sshd (orb handles machine access)
    openssh.enable = false;
  };

  networking = {
    resolvconf.enable = false;
    # Faster DHCP - OrbStack uses SLAAC exclusively
    dhcpcd.extraConfig = ''
      noarp
      noipv6
    '';
  };

  # systemd watchdog disablement
  systemd.services = {
    "systemd-oomd".serviceConfig.WatchdogSec = 0;
    "systemd-userdbd".serviceConfig.WatchdogSec = 0;
    "systemd-udevd".serviceConfig.WatchdogSec = 0;
    "systemd-timesyncd".serviceConfig.WatchdogSec = 0;
    "systemd-timedated".serviceConfig.WatchdogSec = 0;
    "systemd-portabled".serviceConfig.WatchdogSec = 0;
    "systemd-nspawn@".serviceConfig.WatchdogSec = 0;
    "systemd-machined".serviceConfig.WatchdogSec = 0;
    "systemd-localed".serviceConfig.WatchdogSec = 0;
    "systemd-logind".serviceConfig.WatchdogSec = 0;
    "systemd-journald@".serviceConfig.WatchdogSec = 0;
    "systemd-journald".serviceConfig.WatchdogSec = 0;
    "systemd-journal-remote".serviceConfig.WatchdogSec = 0;
    "systemd-journal-upload".serviceConfig.WatchdogSec = 0;
    "systemd-importd".serviceConfig.WatchdogSec = 0;
    "systemd-hostnamed".serviceConfig.WatchdogSec = 0;
    "systemd-homed".serviceConfig.WatchdogSec = 0;
    "systemd-networkd".serviceConfig.WatchdogSec = lib.mkIf config.systemd.network.enable 0;
  };

  # ssh config
  programs.ssh.extraConfig = ''
    Include /opt/orbstack-guest/etc/ssh_config
  '';

  # indicate builder support for emulated architectures
  nix.settings.extra-platforms = [
    "x86_64-linux"
    "i686-linux"
  ];

  users.groups.orbstack.gid = 67278;
}
