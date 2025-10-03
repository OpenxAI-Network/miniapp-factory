{
  inputs = {
    xnode-manager.url = "github:Openmesh-Network/xnode-manager";
    miniapp-factory.url = "github:OpenxAI-Network/miniapp-factory";
    nixpkgs.follows = "miniapp-factory/nixpkgs";
  };

  nixConfig = {
    extra-substituters = [
      "https://openxai.cachix.org"
      "https://nix-community.cachix.org"
      "https://cuda-maintainers.cachix.org"
    ];
    extra-trusted-public-keys = [
      "openxai.cachix.org-1:3evd2khRVc/2NiGwVmypAF4VAklFmOpMuNs1K28bMQE="
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
      "cuda-maintainers.cachix.org-1:0dq3bujKpuEPMCX6U4WylrUDZ9JyUG0VpVZa7CNfq5E="
    ];
  };

  outputs = inputs: {
    nixosConfigurations.container = inputs.nixpkgs.lib.nixosSystem {
      specialArgs = {
        inherit inputs;
      };
      modules = [
        inputs.xnode-manager.nixosModules.container
        {
          services.xnode-container.xnode-config = {
            host-platform = ./xnode-config/host-platform;
            state-version = ./xnode-config/state-version;
            hostname = ./xnode-config/hostname;
          };
        }
        inputs.miniapp-factory.nixosModules.default
        (
          { pkgs, ... }@args:
          {
            services.miniapp-factory.enable = true;
            services.miniapp-factory.github-token = "";

            services.ollama.acceleration = "cuda";
            hardware.graphics = {
              enable = true;
              extraPackages = [
                pkgs.nvidia-vaapi-driver
              ];
            };
            hardware.nvidia.open = true;
            services.xserver.videoDrivers = [ "nvidia" ];

            networking.firewall.allowedTCPPorts = [
              args.config.services.miniapp-factory.port
            ];
          }
        )
      ];
    };
  };
}
