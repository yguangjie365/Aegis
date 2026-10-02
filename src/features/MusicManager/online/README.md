# 在线音乐模块（MU-4.13 / MU-4.14）

## 目录结构

```
src/features/MusicManager/online/
├── pages/
│   └── online-music-page.tsx      # 在线音乐主页面（推荐/歌单/搜索/榜单）
├── components/
│   └── login-modal.tsx             # 登录模态框（多平台选择）
└── store/
    └── online-store.ts             # 在线音乐状态管理（Zustand）
```

## 核心功能

### 1. 在线音乐页面（`online-music-page.tsx`）

从 NexBox `MusicPage.tsx` 提取的核心在线功能，包含四个标签页：

- **推荐**：每日推荐歌曲 + 推荐歌单
- **歌单**：用户个人歌单列表 + 歌单内曲目
- **搜索**：关键词搜索（支持歌曲/歌单）
- **榜单**：各平台热门榜单

#### UI 复用策略

完全复用 NexBox 的组件结构：
- `SongRow` - 歌曲行组件（封面 + 信息 + 时长 + VIP 标识）
- `PlaylistCard` - 歌单卡片组件（封面 + 名称 + 曲目数）

#### 平台支持

- 网易云音乐（netease）
- QQ 音乐（qqmusic）
- 酷狗音乐（kugou）
- 咪咕音乐（migu）
- 汽水音乐（qishui）

### 2. 登录模态框（`login-modal.tsx`）

简化版登录组件，提供多平台选择界面。当前实现为手动输入 Cookie（后续可扩展为自动登录窗口）。

### 3. 状态管理（`online-store.ts`）

基于 Zustand 的状态管理，包含以下核心状态：

#### 登录状态
```typescript
loginStatuses: Record<MusicProvider, LoginInfo | null>
currentPlatform: MusicProvider
```

#### 搜索结果
```typescript
searchResults: SearchResults
searchKeyword: string
searchHistory: string[]
```

#### 歌单
```typescript
userPlaylists: Playlist[]
currentPlaylist: Playlist | null
playlistTracks: Song[]
```

#### 推荐与榜单
```typescript
dailyRecommendations: Song[]
recommendPlaylists: Playlist[]
rankLists: Playlist[]
```

## 后端命令对接

所有 invoke 调用经 Aegis 封装层（`src/lib/tauri/invoke.ts`），使用 `"Music"` 作为 LogModule：

| 命令名 | 参数 | 说明 |
|--------|------|------|
| `music_login_cookie` | `{ platform, cookie }` | Cookie 登录 |
| `music_logout` | `{ platform }` | 退出登录 |
| `music_search` | `{ keywords, platform, limit }` | 搜索歌曲 |
| `music_user_playlists` | `{ platform }` | 获取用户歌单 |
| `music_playlist_tracks` | `{ platform, playlistId }` | 获取歌单曲目 |
| `music_daily_recommend_songs` | `{ platform }` | 每日推荐歌曲 |
| `music_daily_recommend_playlists` | `{ platform }` | 每日推荐歌单 |
| `music_rank_lists` | `{ platform }` | 获取榜单列表 |

## 类型定义

在线音乐相关类型定义在 `src/types/music.ts`：

- `Song` - 歌曲（含 fee/playable/unplayableReason 字段）
- `Playlist` - 歌单
- `LoginInfo` - 登录信息
- `SearchResults` - 搜索结果
- `MusicProvider` - 平台枚举

## 与本地音乐的集成

在线音乐页面通过标签页集成到 `MusicManagerPage`：

```tsx
<Tabs index={activeTab} onChange={setActiveTab}>
  <TabList>
    <Tab>本地音乐</Tab>
    <Tab>在线音乐</Tab>
  </TabList>
  <TabPanels>
    <TabPanel>{/* 本地曲库 */}</TabPanel>
    <TabPanel><OnlineMusicPage /></TabPanel>
  </TabPanels>
</Tabs>
```

## 播放兼容性

由于 `usePlayback.playSong()` 期望 `LocalSong` 类型，在线音乐的 `Song` 需要转换为兼容格式（见 `online-music-page.tsx` 中的 `playSong` 回调）。实际播放时需要后端提供音频 URL。

## 验收标准

- ✅ `npm run build` 编译通过
- ✅ 在线音乐页面能正常渲染
- ✅ 标签页切换正常工作
- ✅ 平台切换触发数据重新加载
- ✅ 搜索功能能调用后端 API
- ✅ 歌单列表能加载并显示

## 后续优化

1. 完善登录流程（自动打开登录窗口而非手动输入 Cookie）
2. 添加歌词同步显示
3. 添加音质选择功能
4. 添加收藏/取消收藏功能
5. 添加评论面板
6. 优化虚拟滚动性能（大量歌曲时）
