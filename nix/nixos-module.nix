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

      gh = lib.mkOption {
        type = lib.types.package;
        default = pkgs.gh;
        example = pkgs.gh;
        description = ''
          gh equivalent executable to use for project creation.
        '';
      };

      git = lib.mkOption {
        type = lib.types.package;
        default = pkgs.git;
        example = pkgs.git;
        description = ''
          git equivalent executable to use for project updates.
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

      github-token = lib.mkOption {
        type = lib.types.str;
        description = ''
          GitHub Access Token for creating repos.
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
        GH_TOKEN = cfg.github-token;
        GH = "${cfg.gh}/bin/";
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

    programs.git = {
      enable = true;
      config = {
        user.name = "Mini App Factory";
        user.email = "miniapp-factory@openxai.org";
        github.user = "miniapp-factory";
        hub.protocol = "ssh";
        init.defaultBranch = "main";
        push.autoSetupRemote = true;
        url."git@github.com:".insteadOf = [
          "https://github.com/"
          "github:"
        ];
        core.sshCommand = "${pkgs.openssh}/bin/ssh -o StrictHostKeyChecking=no -i /var/lib/miniapp-factory/.ssh/id_ed25519";
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
