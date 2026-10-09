from __future__ import annotations

import time
from collections.abc import Sequence

from mega_converter.logging_setup import get_logger
from mega_converter.pipeline.context import PipelineContext
from mega_converter.pipeline.stages import HistoryStage, RulesStage, Stage, StateTransformStage

log = get_logger("pipeline")


class PipelineStopped(RuntimeError):
    pass


DEFAULT_PREVIEW: tuple[Stage, ...] = (HistoryStage(), RulesStage(), StateTransformStage())


def run(ctx: PipelineContext, stages: Sequence[Stage] = DEFAULT_PREVIEW) -> PipelineContext:
    for stage in stages:
        t0 = time.perf_counter()
        stage.run(ctx)
        log.info("stage done", extra={"data": {"stage": stage.name, "ms": round((time.perf_counter() - t0) * 1e3, 1)}})
        if ctx.validation.get("critical", 0) > 0:
            raise PipelineStopped(f"critical validation errors after stage '{stage.name}'")
    return ctx
