import matplotlib.pyplot as plt
import numpy as np
import seaborn as sns

def get_data(filename):
    with open(filename, 'r') as file:
        str = file.read().split(', ')
        data = [int(i) for i in str]
        data = [x for x in data if x < 1200]
        return data

if __name__ == "__main__":
    async_read_time = get_data("async_read_out.txt")
    async_read_std = np.std(async_read_time)
    async_read_avg = np.mean(async_read_time)
    print("async_read_avg: ", async_read_avg, " async_read_std: ", async_read_std)
    box_async_read_time = get_data("box_async_read_out.txt")
    box_async_read_std = np.std(box_async_read_time)
    box_async_read_avg = np.mean(box_async_read_time)
    print("box_async_read_avg: ", box_async_read_avg, " box_async_read_std: ", box_async_read_std)
    sns.kdeplot(async_read_time, color="pink", label="async_read", fill=True, bw_adjust=0.5)
    sns.kdeplot(box_async_read_time, color="blue", label="async_read", fill=True, bw_adjust=0.5)
    plt.title('Probability Density Function (PDF)')
    plt.xlabel('Value')
    plt.autoscale(enable=True, axis='x', tight=None)
    plt.ylabel('Density')
    plt.savefig('test.png')
    plt.show()


