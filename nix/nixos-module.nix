{
  config,
  pkgs,
  lib,
  ...
}:
let
  cfg = config.services.miniapp-factory;
  miniapp-factory = pkgs.callPackage ./package.nix { };
in
{
  options = {
    services.miniapp-factory = {
      enable = lib.mkEnableOption "Enable the mini app factory";

      hostname = lib.mkOption {
        type = lib.types.str;
        default = "0.0.0.0";
        example = "127.0.0.1";
        description = ''
          The hostname under which the app should be accessible.
        '';
      };

      port = lib.mkOption {
        type = lib.types.port;
        default = 54428;
        example = 54428;
        description = ''
          The port under which the app should be accessible.
        '';
      };

      verbosity = lib.mkOption {
        type = lib.types.str;
        default = "warn";
        example = "info";
        description = ''
          The logging verbosity that the app should use.
        '';
      };

      dataDir = lib.mkOption {
        type = lib.types.path;
        default = "/var/lib/miniapp-factory";
        example = "/var/lib/miniapp-factory";
        description = ''
          The main directory to store data.
        '';
      };

      gh = lib.mkOption {
        type = lib.types.package;
        default = pkgs.gh;
        example = pkgs.gh;
        description = ''
          gh equivalent executable to use for project creation.
        '';
      };

      github-token = lib.mkOption {
        type = lib.types.str;
        description = ''
          GitHub Access Token for creating repos.
        '';
      };

      database = lib.mkOption {
        type = lib.types.str;
        default = "postgres:miniapp-factory?host=/run/postgresql";
        example = "postgres:miniapp-factory?host=/run/postgresql";
        description = ''
          Connection string to access the postgres database.
        '';
      };

      rpc = {
        http = lib.mkOption {
          type = lib.types.str;
          default = "https://base-rpc.publicnode.com";
          example = "https://base-sepolia-rpc.publicnode.com";
          description = ''
            Blockchain HTTP RPC to query to smart contract calls.
          '';
        };

        ws = lib.mkOption {
          type = lib.types.str;
          default = "wss://base-rpc.publicnode.com";
          example = "wss://base-sepolia-rpc.publicnode.com";
          description = ''
            Blockchain WebSocket RPC to subscribe to smart contract events.
          '';
        };
      };

      postgres = {
        enable = lib.mkOption {
          type = lib.types.bool;
          default = true;
          example = false;
          description = ''
            Enable the default postgres config.
          '';
        };
      };

      contracts = {
        deposit = lib.mkOption {
          type = lib.types.str;
          default = "0xF0C25895632632047F170Cf4Dda0E41A8BA25789";
          example = "0xF0C25895632632047F170Cf4Dda0E41A8BA25789";
          description = ''
            Mini App Factory monetization contract address. 
          '';
        };

        openx = lib.mkOption {
          type = lib.types.str;
          default = "0xA66B448f97CBf58D12f00711C02bAC2d9EAC6f7f";
          example = "0xEE5b5633B8fa453bD1a4A24973c742BD0488D1C6";
          description = ''
            OPENX contract address. 
          '';
        };
      };

      hyperstackapikey = lib.mkOption {
        type = lib.types.str;
        example = "7a12411b-0074-4d01-a375-ca91376f0bb8";
        description = ''
          The api key to use for hyperstack deployments.
        '';
      };
    };
  };

  config = lib.mkIf cfg.enable {
    users.groups.miniapp-factory = { };
    users.users.miniapp-factory = {
      isSystemUser = true;
      group = "miniapp-factory";
    };

    systemd.services.miniapp-factory = {
      wantedBy = [ "multi-user.target" ];
      description = "AI-powered application to allow creation of Farcaster mini apps with natural language.";
      after = [ "network.target" ];
      environment = {
        HOSTNAME = cfg.hostname;
        PORT = toString cfg.port;
        RUST_LOG = cfg.verbosity;
        DATADIR = cfg.dataDir;
        GH_TOKEN = cfg.github-token;
        GH = "${cfg.gh}/bin/";
        DATABASE = cfg.database;
        HTTPRPC = cfg.rpc.http;
        WSRPC = cfg.rpc.ws;
        DEPOSIT = cfg.contracts.deposit;
        OPENX = cfg.contracts.openx;
        HYPERSTACKAPIKEY = cfg.hyperstackapikey;
      };
      serviceConfig = {
        ExecStart = "${lib.getExe miniapp-factory}";
        User = "miniapp-factory";
        Group = "miniapp-factory";
        StateDirectory = "miniapp-factory";
        Restart = "on-failure";
      };
    };

    systemd.services.miniapp-factory-sshkey = {
      wantedBy = [ "multi-user.target" ];
      description = "Generate SSH key to use for git.";
      serviceConfig = {
        User = "miniapp-factory";
        Group = "miniapp-factory";
        StateDirectory = "miniapp-factory";
      };
      script = ''
        if [ ! -f /var/lib/miniapp-factory/.ssh/id_ed25519 ]; then
          ${pkgs.coreutils}/bin/mkdir /var/lib/miniapp-factory/.ssh
          ${pkgs.openssh}/bin/ssh-keygen -t ed25519 -C \"miniapp-factory@openxai.org\" -f /var/lib/miniapp-factory/.ssh/id_ed25519
        fi
      '';
    };

    services.postgresql = lib.mkIf cfg.postgres.enable {
      enable = true;
      ensureDatabases = [ "miniapp-factory" ];
      ensureUsers = [
        {
          name = "miniapp-factory";
          ensureDBOwnership = true;
        }
      ];
      authentication = pkgs.lib.mkOverride 10 ''
        #type database  DBuser  auth-method
        local sameuser  all     peer
      '';
    };
  };
}
