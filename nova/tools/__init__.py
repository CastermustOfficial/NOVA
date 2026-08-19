"""Tool di NOVA: le mani dell'assistente sul PC."""
from .base import REGISTRY, Risk, ToolError, tool, openai_schema, run_tool

from . import files as _files      # noqa: F401
from . import apps as _apps        # noqa: F401
from . import shell as _shell      # noqa: F401
from . import web as _web          # noqa: F401
from . import system as _system    # noqa: F401
from . import kb as _kb            # noqa: F401

__all__ = ["REGISTRY", "Risk", "ToolError", "tool", "openai_schema", "run_tool"]
