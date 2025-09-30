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

      projectsDir = lib.mkOption {
        type = lib.types.path;
        default = "${cfg.dataDir}/projects";
        example = "/var/lib/miniapp-factory/projects";
        description = ''
          The directory to store projects.
        '';
      };

      usersDir = lib.mkOption {
        type = lib.types.path;
        default = "${cfg.dataDir}/users";
        example = "/var/lib/miniapp-factory/users";
        description = ''
          The directory to store users.
        '';
      };

      model = lib.mkOption {
        type = lib.types.str;
        default = "gpt-oss:20b";
        example = "qwen3-coder:30b-a3b-fp16";
        description = ''
          The Ollama-supported LLM to use for code generation. The full list can be found on https://ollama.com/library
        '';
      };

      git = lib.mkOption {
        type = lib.types.package;
        default = pkgs.git;
        example = pkgs.git;
        description = ''
          git equivalent executable to use for code generation.
        '';
      };

      aider = lib.mkOption {
        type = lib.types.package;
        default = pkgs.aider-chat;
        example = pkgs.aider-chat;
        description = ''
          aider equivalent executable to use for code generation.
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
        PROJECTSDIR = cfg.projectsDir;
        USERSDIR = cfg.usersDir;
        MODEL = cfg.model;
        GIT = "${cfg.git}/bin/";
        AIDER = "${cfg.aider}/bin/";
      };
      serviceConfig = {
        ExecStart = "${lib.getExe miniapp-factory}";
        User = "miniapp-factory";
        Group = "miniapp-factory";
        StateDirectory = "miniapp-factory";
        Restart = "on-failure";
      };
    };

    nixpkgs.config.allowUnfree = true;
    systemd.services.ollama.serviceConfig.DynamicUser = lib.mkForce false;
    systemd.services.ollama.serviceConfig.ProtectHome = lib.mkForce false;
    systemd.services.ollama.serviceConfig.StateDirectory = [ "ollama/models" ];
    services.ollama = {
      enable = true;
      user = "ollama";
      loadModels = [ cfg.model ];
      environmentVariables = {
        OLLAMA_CONTEXT_LENGTH = "8192"; # From https://aider.chat/docs/llms/ollama.html#ollama
      };
    };
    systemd.services.ollama-model-loader.serviceConfig.User = "ollama";
    systemd.services.ollama-model-loader.serviceConfig.Group = "ollama";
    systemd.services.ollama-model-loader.serviceConfig.DynamicUser = lib.mkForce false;
  };
}
