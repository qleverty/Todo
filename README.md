![](https://raw.githubusercontent.com/qleverty/pics/main/todo.png)

A small pet project for CLI task management, written in Rust.

Uses the [todo.txt](http://todotxt.org/) file format, so if you're already using another tool based on the same format — it should be compatible.

Stores tasks in a plain `todo.txt` next to the executable (or in the current directory if one already exists there).

---

## Installation

Download the [installer](https://github.com/qleverty/todo/releases/latest) from the releases page, or build it yourself if you have Rust installed:
```bash
cargo build --release
```

---

## Usage

### Add a task

```bash
todo buy milk                  # no priority
todo A submit the report       # high priority (red)
todo B call the doctor         # medium priority (yellow)
todo C clean up inbox          # low priority (green)
```

### View tasks

```bash
todo list   # or just l
```

Tasks are sorted by priority: A first, then B, C, and no priority last.

### Mark as completed

```bash
todo do 3          # complete task #3
todo d 1 5 9       # multiple tasks at once
todo d 4-7         # range: №4, №5, №6, №7
todo d 1 4-7 10    # mix of both
```

### Delete a task

```bash
todo delete 5      # or del
todo del 1-3 8     # ranges work here too
```

### Edit a task
```bash
todo edit 3 B new text    # change priority and text
todo e 3 B                # change only priority
todo e 3 new text         # remove priority, change text
todo e 3 -                # remove priority only
```

### Re-sort tasks
```bash
todo resort
```

If you manually edited `todo.txt` and messed up the order, this command will restore proper priority sorting (A → B → C → no priority). Note that task IDs may change after resorting.

### Remove completed tasks

```bash
todo clear   # or clr — deletes all completed tasks
```

### Help

```bash
todo help   # or h
```

### Update
```bash
todo update
```

---

## todo.txt format

Plain text file, one task per line:

```
(A) urgent thing
(B) less urgent thing
(C) someday maybe
a task with no priority
x (B) already completed task
```

