# Mach3 设置说明（泡沫切割）

本程序输出的 `.tap` 是标准 Mach3 G 代码。以下是第一次接线 / 调试时的建议步骤。不同机器、不同 breakout board 的引脚不同，请以你的电气图为准。

## 1. 轴

### 两轴平切
- X、Y 接工作台两个方向。
- 热丝上下导向固定，只做平面轮廓。

### 四轴（X/Y + A/B）
- X、Y：下端（或工作台）两个方向。
- A、B：上端导向两个方向。
- 在 Mach3 的 **Config → Ports and Pins → Motor Outputs** 中启用 A、B 轴。
- 在 **Config → Slave Axis**（或 Motor Tuning / 从轴映射）里按你的机械结构把 A/B 与 X/Y 的从轴关系配好；如果 A/B 是独立的上导向电机，则不要从动，直接按上导向的丝杆螺距设置 Steps per。
- 本程序把 A/B 当作“上端坐标”，例如 `A = X + 上端X偏移`。如果机械方向相反，可在 Mach3 的 **Config → Ports and Pins → Motor Outputs** 里把 A/B 的 `Dir LowActive` 改一下，或者在 Mach3 里用反向量。

## 2. 单位与代码格式

程序头部：

```gcode
G21 G90 G94 G17 G40 G49 G80
G91.1
```

- `G21`：毫米。
- `G90`：绝对坐标。
- `G94`：每分钟进给。
- `G17`：XY 平面圆弧。
- `G40/G49/G80`：取消刀补、长度补偿、固定循环。
- `G91.1`：圆弧 I/J 为相对圆心（Mach3 默认）。如果报警圆弧半径不符，先检查这一项和 I/J 数值。

> 本程序已经在轮廓上做了补偿（按割缝宽度一半偏移），所以代码里用 `G40`，不需要 Mach3 再开刀补。

## 3. 热丝开关 M3 / M5

程序用：

```gcode
M03
G04 P0.3
...
M05
```

控制热丝。Mach3 里通常把 `M3` 映射到主轴输出 / 继电器输出：

1. **Config → Ports and Pins → Spindle Setup**：
   - 勾选 **Use Spindle Motor Output**（如果使用主轴继电器）。
   - 选择 **Step/Dir** 或 **PWM** 按你的板子；如果只是继电器，可用 **Relay Outputs**。
2. **Config → Ports and Pins → Output Signals**：
   - 把 `Output #1` 映射到你接热丝继电器的并口 / 板卡引脚。
3. 在 **Config → Ports and Pins → Spindle Setup → Relay Control** 或 **Output Signals** 中把 `M3` 设为 `Output #1`。
4. 先在 **Diagnostics** 页面按 `M3` / `M5`，看对应 LED 是否亮灭；用万用表确认继电器动作。
5. 热丝电源建议经接触器 / 固态继电器隔离，不要直接用板卡输出带大电流。

`G04 P0.3` 是起弧等待 0.3 秒。泡沫太厚或热丝升温慢时，可在代码里把 `P0.3` 改大（例如 `P1.0`）。

## 4. 进给与速度

- 切割速度 `F` 是 mm/min，直接在程序里输出。
- 空走速度在程序里用 `G00`，Mach3 的 **Motor Tuning → Velocity** 决定实际快移速度。
- 第一次试切建议把 Mach3 的 **Feedrate Override** 降到 30%～50%，确认方向后再恢复。
- 泡沫切割的进给与热丝温度、泡沫密度、厚度关系很大；建议先切废料，记录一组适合你材料的 `F` 值。

## 5. 软限位与回零

- 泡沫块尺寸通常比工作台小，建议在 Mach3 里设置 **Soft Limits**，防止空走撞到夹具。
- 如果 A/B 是上导向，回零顺序要先回上导向再回 X/Y，避免热丝被拉弯。
- 程序结束会 `M05` 关丝并 `G00` 回到起点；请确认回起点路径不会撞到泡沫或夹具。

## 6. 第一次运行检查表

- [ ] 热丝电源断开，或热丝不接电。
- [ ] Mach3 急停可用。
- [ ] 程序单位显示为 mm。
- [ ] X/Y 手动点动方向与图纸一致。
- [ ] A/B（四轴）手动点动方向与上导向一致。
- [ ] `M3` 时继电器吸合，`M5` 时释放。
- [ ] 单段执行前 20 行，观察 `G00` 起点和 `G01` 引入线。
- [ ] 圆弧段 `G02/G03` 转向正确，没有 “Radius to end of arc differs” 报警。
- [ ] 割缝补偿在废料侧，切出的零件尺寸合格。
- [ ] 确认后接通热丝，先用 30% 进给试切。

## 7. 常见报警

| 报警 / 现象 | 可能原因 | 处理 |
| --- | --- | --- |
| `Radius to end of arc differs from radius to start` | I/J 模式不对，或圆弧被后处理成整圆 | 保留 `G91.1`；检查程序圆弧 I/J；必要时把整圆拆成两段半圆 |
| 轴不动，但程序在走 | 轴未启用 / 脉冲方向接错 / 驱动报警 | 检查 Ports and Pins、驱动器使能、急停 |
| `M3` 没有输出 | 主轴 / 继电器未映射 | 按第 3 节映射 Output #1，Diagnostics 测试 |
| 切出来尺寸偏大 | 割缝补偿方向反了 / 补偿量偏小 | 改“内/外轮廓”或加大割缝宽；先切废料 |
| A/B 不动或方向反 | 四轴未启用 / 从轴未配置 / Dir 反向 | 检查 Motor Outputs、Slave Axis、Dir LowActive |
| 空走撞夹具 | 起点或软限位设置不当 | 改起点 X/Y，设置 Soft Limits，降低快移速度 |

## 8. 参考程序

```gcode
%
(CHAOYUE CNC FOAM CUT / MACH3)
(PART SAMPLE)
(MODE 2-AXIS X Y)
(KERF 0.400 MM  COMP 0.200 MM  FEED 900 MM/MIN  LEAD 3.0 MM)
(CUT 989.2 MM  TRAVEL 230.8 MM  EST 1.2 MIN)
(M03 = HOT WIRE ON, M05 = HOT WIRE OFF)
G21 G90 G94 G17 G40 G49 G80
G91.1
M05
G00 X-122.000 Y-82.000
M03
G04 P0.3
G01 X-119.000 Y-79.000 F900.000
G01 X-110.000 Y-70.000 F900.000
...
M05
G00 X-122.000 Y-82.000
M30
%
```

实际程序以软件生成为准；上面的坐标只是示意。
