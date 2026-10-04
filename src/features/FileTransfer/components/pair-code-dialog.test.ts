import { describe, expect, it } from "vitest";
import { PAIR_CODE_LENGTH, normalizePairCode } from "../components/pair-code-dialog";

describe("normalizePairCode（用户从聊天软件复制时的清洗）", () => {
  it("去空白并转大写", () => {
    expect(normalizePairCode(" k7m2q9 ")).toBe("K7M2Q9");
    expect(normalizePairCode("K7 M2 Q9")).toBe("K7M2Q9");
    expect(normalizePairCode("\tK7M2Q9\n")).toBe("K7M2Q9");
    // 全角空格（中文输入法下常见）
    expect(normalizePairCode("K7M2\u3000Q9")).toBe("K7M2Q9");
  });

  it("🔴 全角字母数字转半角（中文输入法默认全角的场景）", () => {
    expect(normalizePairCode("Ｋ７Ｍ２Ｑ９")).toBe("K7M2Q9");
    expect(normalizePairCode("ｋ７ｍ２ｑ９")).toBe("K7M2Q9");
  });

  it("长度与字母表口径：清洗后应正好 6 位", () => {
    expect(normalizePairCode(" K 7 M 2 Q 9 ")).toHaveLength(PAIR_CODE_LENGTH);
  });

  it("不改动非法字符（交给后端判，前端不自行解释）", () => {
    expect(normalizePairCode("K7-M2Q")).toBe("K7-M2Q");
    expect(normalizePairCode("!!!!!!")).toBe("!!!!!!");
    expect(normalizePairCode("")).toBe("");
  });
});
