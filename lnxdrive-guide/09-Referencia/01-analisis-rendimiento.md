# Resumen del Análisis de Rendimiento: Rust vs C# .NET

> **Ubicación:** `09-Referencia/01-analisis-rendimiento.md`
> **Relacionado:** [Anexo B completo](../Anexos/B-benchmarks-rust-vs-dotnet.md), [Justificación de Rust](../05-Implementacion/02-justificacion-rust.md)

---

## Resumen Ejecutivo

Este documento resume el análisis técnico de rendimiento comparando **Rust** vs **C# .NET** para los componentes de sistema de LNXDrive (daemon, FUSE filesystem, CLI).

### Conclusión Principal

| Aspecto | Ganancia de Rust vs .NET | Criticidad para LNXDrive |
|---------|--------------------------|--------------------------|
| **Latencia FUSE** | 10-100x menor | **CRÍTICA** |
| **Consumo de memoria** | 3-10x menor | Alta |
| **Tiempo de inicio CLI** | 10-50x menor | Media |
| **Tamaño de binarios** | 5-20x menor | Baja |
| **Predictibilidad** | Sin pausas GC | **CRÍTICA** para FUSE |
| **Throughput sostenido** | 1.2-2x mejor | Media |

**Recomendación**: Rust es significativamente superior para los componentes de sistema de LNXDrive, especialmente para FUSE donde la latencia y predictibilidad son críticas.

---

## Comparación de Modelos de Ejecución

### Rust
- Compilación ahead-of-time (AOT) siempre
- Optimizaciones agresivas en tiempo de compilación
- Sin runtime, sin Garbage Collector
- El binario ES el programa, nada más

### C# .NET
- Compilación a IL (bytecode) + JIT en runtime
- GC gestiona memoria con pausas impredecibles
- Native AOT (.NET 8+) mejora startup pero mantiene GC

---

## Gestión de Memoria

### Rust: Ownership System (Zero-Cost)
```rust
fn process_file(path: PathBuf) -> Result<()> {
    let content = fs::read(&path)?;  // Memoria allocada
    let hash = compute_hash(&content);
    // ...
}  // ← content se libera AQUÍ, determinísticamente
   //   Sin GC, sin pausa, sin overhead
```

### C# .NET: Garbage Collector
```csharp
void ProcessFile(string path) {
    var content = File.ReadAllBytes(path);  // Memoria en heap
    var hash = ComputeHash(content);
    // ...
}  // ← content se marca como "no referenciado"
   //   Se libera EVENTUALMENTE cuando GC decida
```

**Pausas típicas de GC:**
- Gen 0: ~1ms
- Gen 1: ~10ms
- Gen 2: ~50-200ms

---

## Benchmarks de Referencia

### TechEmpower Framework Benchmarks (Round 21)

| Benchmark | Rust (actix) | C# (ASP.NET) | Diferencia |
|-----------|--------------|--------------|------------|
| JSON serialization | 847,000 req/s | 512,000 req/s | Rust +65% |
| Single query | 723,000 req/s | 498,000 req/s | Rust +45% |
| Multiple queries | 98,000 req/s | 67,000 req/s | Rust +46% |
| Plaintext | 7,100,000 req/s | 4,200,000 req/s | Rust +69% |

### Consumo de Memoria

| Métrica | Rust | C# .NET 8 | C# Native AOT |
|---------|------|-----------|---------------|
| Memoria inicial (idle) | 2-5 MB | 30-50 MB | 15-25 MB |
| Memoria bajo carga | 10-20 MB | 80-150 MB | 50-80 MB |

### Tiempo de Inicio

| Aplicación | Rust | C# JIT | C# Native AOT |
|------------|------|--------|---------------|
| Hello World | 1-2 ms | 50-80 ms | 10-15 ms |
| CLI simple | 5-10 ms | 100-200 ms | 20-40 ms |
| Daemon HTTP | 10-20 ms | 300-500 ms | 50-100 ms |

---

## Impacto en FUSE (CRÍTICO)

El componente FUSE es donde las diferencias son más críticas:

### El Problema del GC en FUSE

```
Aplicación (Nautilus)
       │
       │ open("/mnt/lnxdrive/doc.pdf")
       ▼
   Kernel Linux (VFS layer)
       │
       │ FUSE protocol (via /dev/fuse)
       ▼
   lnxdrive fuse daemon  ← AQUÍ ES DONDE IMPORTA EL LENGUAJE
                           Debe responder en MICROSEGUNDOS
```

**Problema con GC:**
- El kernel ESPERA respuesta del daemon FUSE
- Si el GC pausa 50-200ms, el kernel puede hacer timeout
- Aplicaciones que acceden al archivo se BLOQUEAN
- En casos extremos: procesos en estado "D" (uninterruptible)

### Benchmark de Operaciones FUSE

| Operación | Rust (fuser) | C# (Tmds.Fuse) | Diferencia |
|-----------|--------------|----------------|------------|
| getattr() | 0.5-2 μs | 10-50 μs + GC | Rust 10-100x mejor |
| readdir() | 5-20 μs | 50-200 μs + GC | Rust 10-40x mejor |
| open() | 1-5 μs | 20-100 μs + GC | Rust 20-50x mejor |
| read() (cached) | 2-10 μs | 30-150 μs + GC | Rust 15-50x mejor |

---

## Escenario Real: Usuario con 50,000 archivos

**Operación: Abrir carpeta con 500 archivos en Nautilus**

### Rust
- 1 × readdir() = 20 μs
- 500 × getattr() = 500 × 2 μs = 1 ms
- **Total: ~1.02 ms** → Percepción usuario: INSTANTÁNEO

### C# .NET (mejor caso, sin GC)
- 1 × readdir() = 100 μs
- 500 × getattr() = 500 × 30 μs = 15 ms
- **Total: ~15.1 ms** → Percepción usuario: Rápido pero medible

### C# .NET (caso con GC Gen2)
- Operaciones + GC pause de 100ms
- **Total: ~115 ms** → Percepción usuario: LAG NOTABLE

---

## Matriz de Decisión Final

| Factor | Peso | Rust | C# .NET | Rust Ponderado | C# Ponderado |
|--------|------|------|---------|----------------|--------------|
| Latencia FUSE | 30% | 10 | 3 | 3.0 | 0.9 |
| Predictibilidad (sin GC) | 25% | 10 | 2 | 2.5 | 0.5 |
| Consumo memoria | 15% | 10 | 4 | 1.5 | 0.6 |
| Tiempo inicio CLI | 10% | 10 | 5 | 1.0 | 0.5 |
| Ecosistema/productividad | 10% | 6 | 9 | 0.6 | 0.9 |
| Curva aprendizaje | 5% | 4 | 8 | 0.2 | 0.4 |
| SDK Graph oficial | 5% | 5 | 10 | 0.25 | 0.5 |
| **TOTAL** | 100% | - | - | **9.05** | **4.3** |

---

## Conclusión

**RECOMENDACIÓN: RUST PARA TODOS LOS COMPONENTES DE SISTEMA**

### Razones principales:

1. **FUSE REQUIERE LATENCIA PREDECIBLE**
   - El GC de .NET es incompatible con los requisitos de FUSE
   - No hay workaround: incluso Native AOT tiene GC
   - Una pausa de 100ms en FUSE = mala experiencia de usuario

2. **DAEMON LONG-RUNNING SE BENEFICIA DE MEMORIA CONTROLADA**
   - 5 MB vs 50 MB hace diferencia en laptops
   - Sin picos de memoria por GC
   - Mejor para batería

3. **CLI SE BENEFICIA DE INICIO RÁPIDO**
   - 5ms vs 150ms es diferencia perceptible
   - Usuarios ejecutan `lnxdrive status` frecuentemente

4. **CONSISTENCIA DE STACK**
   - Un lenguaje para daemon + FUSE + CLI
   - Compartir código entre componentes
   - Un solo set de herramientas de desarrollo

---

## Ver también

- [Anexo B: Análisis completo Rust vs .NET](../Anexos/B-benchmarks-rust-vs-dotnet.md) - Documento técnico detallado
- [Justificación de Rust](../05-Implementacion/02-justificacion-rust.md) - Por qué Rust para LNXDrive
- [Files-on-Demand con FUSE](../04-Componentes/01-files-on-demand-fuse.md) - Implementación del filesystem
- [Stack Tecnológico](../05-Implementacion/01-stack-tecnologico.md) - Tecnologías seleccionadas
