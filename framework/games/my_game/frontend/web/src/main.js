const board = Array(9).fill("");
let next = "X";

function winner() {
  const lines = [
    [0, 1, 2],
    [3, 4, 5],
    [6, 7, 8],
    [0, 3, 6],
    [1, 4, 7],
    [2, 5, 8],
    [0, 4, 8],
    [2, 4, 6],
  ];
  for (const [a, b, c] of lines) {
    if (board[a] && board[a] === board[b] && board[a] === board[c]) return board[a];
  }
  return "";
}

function render() {
  const app = document.getElementById("app");
  const win = winner();
  app.innerHTML = `
    <h2>Tic-Tac-Toe (3x3)</h2>
    <p>${win ? `Winner: ${win}` : `Next: ${next}`}</p>
    <div style="display:grid;grid-template-columns:repeat(3,64px);gap:6px;">
      ${board
        .map(
          (cell, i) =>
            `<button data-i="${i}" style="width:64px;height:64px;font-size:28px;">${cell}</button>`
        )
        .join("")}
    </div>`;
  for (const btn of app.querySelectorAll("button[data-i]")) {
    btn.addEventListener("click", () => {
      const idx = Number(btn.dataset.i);
      if (board[idx] || winner()) return;
      board[idx] = next;
      next = next === "X" ? "O" : "X";
      render();
    });
  }
}

render();
