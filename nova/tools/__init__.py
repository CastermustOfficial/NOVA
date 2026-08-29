"""Tool di NOVA: le mani dell'assistente sul PC."""
from .base import REGISTRY, Risk, ToolError, tool, openai_schema, run_tool

from . import files as _files      # noqa: F401
from . import apps as _apps        # noqa: F401
from . import shell as _shell      # noqa: F401
from . import web as _web          # noqa: F401
from . import system as _system    # noqa: F401
from . import kb as _kb            # noqa: F401
from . import deleghe as _deleghe  # noqa: F401
from . import schermo as _schermo  # noqa: F401
from . import documenti as _documenti  # noqa: F401
from . import tempo as _tempo      # noqa: F401
from . import riparazione as _riparazione  # noqa: F401
from . import procedure as _procedure  # noqa: F401
# Per ultimo: registra anche le automazioni gia' salvate, che diventano
# strumenti a tutti gli effetti accanto a quelli scritti a mano.
from . import automazioni as _automazioni  # noqa: F401

__all__ = ["REGISTRY", "Risk", "ToolError", "tool", "openai_schema", "run_tool"]
