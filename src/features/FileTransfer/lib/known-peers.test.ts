import { describe, expect, it } from "vitest";
import {
  MAX_PORT,
  MIN_PORT,
  NEW_DEVICE_NAME,
  addKnownPeer,
  createKnownPeerRow,
  isValidPort,
  isSamePeer,
  removeKnownPeer,
  renameKnownPeer,
} from "./known-peers";
import type { KnownPeer } from "@/types/transfer";

const A: KnownPeer = { name: "书房台式机", ip: "192.168.1.24", port: 18889 };
const B: KnownPeer = { name: "笔记本", ip: "192.168.1.25", port: 18889 };

describe("isValidPort（F-FT-10：端口非法时行内报错、不落库）", () => {
  it("区间内合法", () => {
    expect(isValidPort("1024")).toBe(true);
    expect(isValidPort("18889")).toBe(true);
    expect(isValidPort("65535")).toBe(true);
    expect(isValidPort(" 18889 ")).toBe(true);
  });

  it("区间外非法", () => {
    expect(isValidPort("1023")).toBe(false);
    expect(isValidPort("0")).toBe(false);
    expect(isValidPort(String(MAX_PORT + 1))).toBe(false);
    expect(isValidPort(String(MIN_PORT - 1))).toBe(false);
  });

  it("🔴 只接受纯数字（parseInt 会把 12abc 当 12，那是把笔误当合法）", () => {
    expect(isValidPort("12abc")).toBe(false);
    expect(isValidPort("18 889")).toBe(false);
    expect(isValidPort("-1")).toBe(false);
    expect(isValidPort("")).toBe(false);
    expect(isValidPort("abc")).toBe(false);
    expect(isValidPort("1.5")).toBe(false);
  });
});

describe("isSamePeer（按 IP + 端口，不看名称）", () => {
  it("IP 与端口都相同即同一条，名称无关", () => {
    expect(isSamePeer(A, { ...A, name: "改了名字" })).toBe(true);
  });

  it("IP 或端口任一不同即两条", () => {
    expect(isSamePeer(A, { ...A, ip: "192.168.1.99" })).toBe(false);
    expect(isSamePeer(A, { ...A, port: 19000 })).toBe(false);
  });

  it("IP 两侧空白不影响判定", () => {
    expect(isSamePeer(A, { ...A, ip: " 192.168.1.24 " })).toBe(true);
  });
});

describe("addKnownPeer（F-FT-10：重复加入不产生第二条）", () => {
  it("新地址加入成功", () => {
    const { list, added } = addKnownPeer([A], B);
    expect(added).toBe(true);
    expect(list).toHaveLength(2);
    expect(list[1]).toEqual(B);
  });

  it("🔴 同 IP+端口重复加入 => 不新增，且**保留原有名称**", () => {
    const { list, added } = addKnownPeer([A], { ...A, name: "又一次加入" });
    expect(added).toBe(false);
    expect(list).toHaveLength(1);
    expect(list.at(0)?.name).toBe("书房台式机");
  });

  it("空清单也能加", () => {
    expect(addKnownPeer([], A).list).toEqual([A]);
  });

  it("不修改入参数组（纯函数）", () => {
    const original = [A];
    addKnownPeer(original, B);
    expect(original).toHaveLength(1);
  });
});

describe("removeKnownPeer", () => {
  it("按 IP+端口移除", () => {
    expect(removeKnownPeer([A, B], A)).toEqual([B]);
  });

  it("不存在时原样返回", () => {
    expect(removeKnownPeer([A], B)).toEqual([A]);
  });

  it("同名不同地址只移除目标那条", () => {
    const twin: KnownPeer = { ...A, ip: "10.0.0.1" };
    expect(removeKnownPeer([A, twin], A)).toEqual([twin]);
  });
});

describe("renameKnownPeer（F-FT-10：编辑仅作用于选中行）", () => {
  it("只改定位到的那条", () => {
    const list = renameKnownPeer([A, B], B, "新名字");
    expect(list.at(0)?.name).toBe("书房台式机");
    expect(list.at(1)?.name).toBe("新名字");
  });

  it("未命中则原样返回", () => {
    expect(renameKnownPeer([A], B, "x")).toEqual([A]);
  });
});

describe("createKnownPeerRow（F-FT-10：默认命名「新设备」并自动选中该行）", () => {
  it("追加一行并返回它的索引（供调用方选中）", () => {
    const { list, index } = createKnownPeerRow([A, B]);
    expect(list).toHaveLength(3);
    expect(index).toBe(2);
    expect(list[index]?.name).toBe(NEW_DEVICE_NAME);
  });

  it("🔴 默认端口是合法的（不能给 0，否则一添加就处于行内报错态）", () => {
    const { list, index } = createKnownPeerRow([]);
    expect(isValidPort(String(list[index]?.port))).toBe(true);
  });

  it("空清单时索引为 0", () => {
    expect(createKnownPeerRow([]).index).toBe(0);
  });
});
