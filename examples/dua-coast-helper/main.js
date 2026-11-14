export function setup(ctx) {
  ctx.ui([
    { type: "button", label: "바닷가로 이동", run: () => ctx.dua.go("coast") },
    { type: "button", label: "현재 낚시 장소 기억", run: () => ctx.dua.mark("coast") },
    { type: "button", label: "상태 확인", run: async () => {
      const state = await ctx.dua.status();
      ctx.log(state.message);
    } },
    { type: "button", label: "화면 확인", run: async () => {
      const frame = await ctx.dua.observe();
      ctx.log(`${frame.image.width}×${frame.image.height} · OCR ${frame.words.length}`);
    } },
    { type: "button", label: "정지", run: () => ctx.dua.stop() },
  ]);
}
