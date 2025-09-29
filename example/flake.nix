{
  inputs = {
    xnode-manager.url = "github:Openmesh-Network/xnode-manager";
    xnode-rust-template.url = "github:Openmesh-Network/xnode-rust-template"; # "path:..";
    nixpkgs.follows = "xnode-rust-template/nixpkgs";
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
        inputs.xnode-rust-template.nixosModules.default
        {
          services.xnode-rust-template.enable = true;
        }
      ];
    };
  };
}
